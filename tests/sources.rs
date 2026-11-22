mod common;

use std::sync::atomic::Ordering;

use common::{Highlights, at, genesis};
use klause::{Error, Layer, Span, SpanRef, Value, span_id, spans_in};

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
    let verse = || doc.get_passage(span_id("verses", "GEN 1:1")).unwrap();

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
            .within(spans_in("highlights").with_attr("color", "green"))
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

    let passage = doc.get_passage(span_id("verses", "GEN 1:1")).unwrap();
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

    let passage = doc.get_passage(span_id("verses", "GEN 1:1")).unwrap();
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

#[test]
fn query_sources_are_read_at_the_final_verb_not_during_building_or_planning() {
    let (doc, highlights) = with_highlights();
    let query = doc.find().with_text("God").in_layer("highlights");
    highlights.save(Span::from(at("God", 0)).with_id(1_u64));
    assert_eq!(query.get_selection().unwrap().ranges(), [at("God", 0)]);

    let query = doc.find().in_layer("highlights");
    highlights.down.store(true, Ordering::SeqCst);
    assert!(matches!(query.count(), Err(Error::Source { .. })));
    assert!(matches!(
        doc.find()
            .within(Vec::<std::ops::Range<u32>>::new())
            .in_layer("highlights")
            .count(),
        Err(Error::Source { .. })
    ));
}

#[test]
fn source_range_only_queries_still_normalize_and_validate() {
    let (doc, highlights) = with_highlights();
    highlights.save(Span::from(vec![1..1, 1..3]).with_id(1_u64));
    highlights.save(Span::from(3..3).with_id(2_u64));
    highlights.save(Span::from(4..5)); // Anonymous source spans are skipped.
    let filter = || spans_in("highlights");
    assert_eq!(doc.find().count_spans(filter()).unwrap(), 2);
    assert_eq!(doc.find().within(1..1).count_spans(filter()).unwrap(), 0);
    assert_eq!(doc.find().within(3..3).count_spans(filter()).unwrap(), 1);

    let invalid = 0..doc.len() + 1;
    highlights.save(Span::from(invalid.clone()).with_id(3_u64));
    assert!(matches!(
        doc.find().within(Vec::<std::ops::Range<u32>>::new()).count_spans(filter()),
        Err(Error::OutOfRange(range)) if range == invalid
    ));
}
