mod common;

use std::sync::atomic::Ordering;

use common::{Highlights, at, genesis};
use klause::{Error, Layer, Span, SpanRef, Value, select_span, select_spans};

fn with_highlights() -> (klause::Document, Highlights) {
    let mut doc = genesis();
    let highlights = Highlights::default();
    doc.add_layer(Layer::source("highlights", highlights.clone()))
        .unwrap();
    (doc, highlights)
}

#[test]
fn source_spans_are_read_fresh_for_every_passage_and_query() {
    let (doc, highlights) = with_highlights();
    highlights.save(
        Span::from(at("God", 0))
            .with_id(1_u64)
            .with_attr("color", "yellow"),
    );
    let verse = || doc.get_passage(select_span("verses", "GEN 1:1")).unwrap();

    let before = verse();
    assert_eq!(before.spans_in("highlights").count(), 1);

    highlights.save(
        Span::from(at("heavens", 0))
            .with_id(2_u64)
            .with_attr("color", "green"),
    );
    assert_eq!(verse().spans_in("highlights").count(), 2);
    // A passage already built doesn't change.
    assert_eq!(before.spans_in("highlights").count(), 1);

    assert_eq!(
        doc.find()
            .in_layer("highlights")
            .get_selection()
            .unwrap()
            .ranges(),
        [at("God", 0), at("heavens", 0)]
    );
    assert_eq!(
        doc.find()
            .within(select_spans("highlights").with_attr("color", "green"))
            .get_spans("words")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn source_spans_can_target_stored_spans_and_missing_targets_are_skipped() {
    let (doc, highlights) = with_highlights();
    highlights.save(Span::on("words", 5_u64).with_id(1_u64));
    highlights.save(Span::on("words", 99_u64).with_id(2_u64));

    let passage = doc.get_passage(select_span("verses", "GEN 1:1")).unwrap();
    let spans: Vec<_> = passage.spans_in("highlights").collect();

    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text(), ["created"]);
    assert!(passage.error("highlights").is_none());
}

#[test]
fn source_layers_are_read_only() {
    let (mut doc, highlights) = with_highlights();
    highlights.save(Span::from(0..2).with_id(1_u64));
    let saved = SpanRef::new("highlights", 1_u64);

    let layer = doc.layer("highlights").unwrap();
    assert!(layer.is_source());
    assert_eq!(layer.spans().count(), 0);
    assert!(matches!(
        doc.add_span("highlights", Span::from(3..6)),
        Err(Error::ReadOnlyLayer(id)) if id == "highlights"
    ));
    assert!(matches!(
        doc.replace_span(&saved, Span::from(3..6)),
        Err(Error::ReadOnlyLayer(_))
    ));
    assert!(matches!(
        doc.remove_span(&saved),
        Err(Error::ReadOnlyLayer(_))
    ));
    assert_eq!(highlights.rows.lock().unwrap().len(), 1);
}

#[test]
fn a_failing_source_still_loads_passages_but_fails_queries_that_need_it() {
    let (doc, highlights) = with_highlights();
    highlights.save(
        Span::from(at("God", 0))
            .with_id(1_u64)
            .with_attr("color", "yellow"),
    );
    highlights.down.store(true, Ordering::SeqCst);

    let passage = doc.get_passage(select_span("verses", "GEN 1:1")).unwrap();
    assert!(passage.error("highlights").is_some());
    assert!(passage.error("words").is_none());
    assert_eq!(passage.spans_in("highlights").count(), 0);
    assert_eq!(passage.spans_in("words").count(), 10);
    assert_eq!(
        passage.spans_in("morph").next().unwrap().attr("pos"),
        Some(&Value::from("prep"))
    );

    assert!(matches!(
        doc.find().in_layer("highlights").count(),
        Err(Error::Source { layer, .. }) if layer == "highlights"
    ));
    assert_eq!(doc.find().in_layer("words").count().unwrap(), 17);
}
