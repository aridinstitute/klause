mod common;

use common::{GENESIS, at, genesis, verse};
use klause::{Document, Error, Passage, Span, Value, span_id, spans_in};

fn layers(passage: &Passage) -> Vec<&str> {
    let mut layers: Vec<&str> = passage.spans().map(|span| span.layer()).collect();
    layers.dedup();
    layers
}

#[test]
fn a_passage_holds_the_spans_that_touch_it_clipped_to_its_text() {
    let doc = genesis();
    // From the space after the first "the" to the middle of "created".
    let passage = doc.get_passage(6..24).unwrap();

    assert_eq!(passage.range(), 6..24);
    assert_eq!(passage.text(), " beginning God cre");
    // "the" ends where the passage starts, so it doesn't touch it.
    assert_eq!(
        passage
            .spans_in("words")
            .map(|span| (span.local().to_vec(), span.text()))
            .collect::<Vec<_>>(),
        [
            (vec![1..10], vec!["beginning"]),
            (vec![11..14], vec!["God"]),
            (vec![15..18], vec!["cre"]),
        ]
    );

    let created = passage.spans_in("words").last().unwrap();
    assert!(created.starts_inside());
    assert!(!created.ends_inside());
    let verse = passage.spans_in("verses").next().unwrap();
    assert_eq!(verse.local(), [0..18]);
    assert!(!verse.starts_inside());
    assert!(!verse.ends_inside());
    assert_eq!(passage.spans_in("notes").count(), 0);
}

#[test]
fn spans_that_target_included_spans_are_included_in_layer_then_start_order() {
    let doc = genesis();
    let passage = doc.get_passage(6..24).unwrap();

    assert_eq!(
        layers(&passage),
        ["books", "chapters", "verses", "words", "morph"]
    );
    let tags: Vec<_> = passage.spans_in("morph").collect();
    assert_eq!(
        tags.iter().map(|tag| tag.attr("pos")).collect::<Vec<_>>(),
        [
            Some(&Value::from("noun")),
            Some(&Value::from("noun")),
            Some(&Value::from("verb"))
        ]
    );
    assert_eq!(tags[2].local(), [15..18]);
    assert_eq!(tags[2].to_ref().layer(), "morph");
}

#[test]
fn points_touch_a_passage_at_or_between_its_ends() {
    let doc = genesis();
    let note = at(".", 0).end;
    let earth = at("earth", 0).start;

    let ending_at_note = doc.get_passage(earth..note).unwrap();
    let notes: Vec<_> = ending_at_note.spans_in("notes").collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].local(), [note - earth..note - earth]);

    assert_eq!(
        doc.get_passage(note..note + 4)
            .unwrap()
            .spans_in("notes")
            .count(),
        1
    );
    assert_eq!(
        doc.get_passage(earth..note - 1)
            .unwrap()
            .spans_in("notes")
            .count(),
        0
    );
}

#[test]
fn get_passage_needs_exactly_one_range_and_get_passages_gives_one_per_range() {
    let doc = genesis();

    assert!(matches!(
        doc.get_passage([0..2, 7..16]),
        Err(Error::NotOne { found: 2 })
    ));
    assert!(matches!(
        doc.get_passage(spans_in("morph").with_attr("pos", "pronoun")),
        Err(Error::NotOne { found: 0 })
    ));

    let words = doc.get_passages(spans_in("words")).unwrap();
    assert_eq!(words.len(), 17);
    assert_eq!(
        words.iter().take(3).map(Passage::text).collect::<Vec<_>>(),
        ["In", "the", "beginning"]
    );

    // The verses meet, so they come back as one passage.
    let verses = doc.get_passages(spans_in("verses")).unwrap();
    assert_eq!(verses.len(), 1);
    assert_eq!(verses[0].text(), GENESIS);

    let second = doc.get_passage(span_id("verses", "GEN 1:2")).unwrap();
    assert_eq!(second.range(), verse(2));
    assert_eq!(doc.get_passage(..).unwrap().text(), GENESIS);
}

#[test]
fn a_passage_keeps_its_text_when_the_document_grows_and_can_cross_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Passage>();

    let mut doc = genesis();
    let passage = doc.get_passage(..).unwrap();
    let copy = passage.clone();

    doc.push(" And God said").unwrap();

    assert_eq!(passage.text(), GENESIS);
    assert_eq!(passage.range(), 0..GENESIS.len() as u32);
    assert_eq!(
        std::thread::spawn(move || copy.text().to_owned())
            .join()
            .unwrap(),
        GENESIS
    );
    assert!(
        doc.get_passage(..)
            .unwrap()
            .text()
            .ends_with("void. And God said")
    );
}

#[test]
fn with_layers_limits_which_layers_a_query_passage_holds() {
    let doc = genesis();

    let passage = doc
        .find()
        .in_span("verses", "GEN 1:1")
        .with_layers(["words"])
        .get_passage()
        .unwrap();

    assert_eq!(passage.range(), verse(1));
    assert_eq!(layers(&passage), ["words"]);
    assert_eq!(passage.spans().count(), 10);
}

// Independent scan oracle: clip every resolved range, without a position lookup.
fn assert_passages_match_scan(doc: &Document) {
    for start in 0..=doc.len() {
        for end in start..=doc.len() {
            let mut expected = Vec::new();
            for layer in doc.layers() {
                let mut in_layer = Vec::new();
                for span in layer.spans() {
                    let reference = span_id(layer.id(), span.id().unwrap().clone());
                    let local: Vec<_> = doc
                        .ranges(&reference)
                        .unwrap()
                        .iter()
                        .filter_map(|r| {
                            if r.is_empty() {
                                (start <= r.start && r.start <= end).then_some(
                                    r.start.saturating_sub(start)..r.end.saturating_sub(start),
                                )
                            } else {
                                let a = start.max(r.start);
                                let b = end.min(r.end);
                                (a < b).then(|| a - start..b - start)
                            }
                        })
                        .collect();
                    if !local.is_empty() {
                        in_layer.push((reference, local));
                    }
                }
                in_layer.sort_by_key(|(_, local)| local[0].start);
                expected.extend(in_layer);
            }
            // Run twice to cover both the first lookup and cache reuse.
            for _ in 0..2 {
                let actual: Vec<_> = doc
                    .get_passage(start..end)
                    .unwrap()
                    .spans()
                    .map(|s| (s.to_ref(), s.local().to_vec()))
                    .collect();
                assert_eq!(actual, expected, "passage {start}..{end}");
            }
        }
    }
}

#[test]
fn indexed_passages_match_scans_for_points_nested_and_disjoint_ranges() {
    let mut doc = Document::new("test").with_layer("spans").with_layer("tags");
    doc.push("0123456789abcdef").unwrap();
    // Reverse insertion/ID order exercises sorting and equal starts.
    for start in (0..=16).rev() {
        for end in (start..=16).rev() {
            doc.add_span("spans", Span::from(start..end)).unwrap();
        }
    }
    let split = doc
        .add_span("spans", Span::from([0..2, 7..9, 14..16]))
        .unwrap();
    doc.add_span("spans", Span::from(Vec::<std::ops::Range<u32>>::new()))
        .unwrap();
    doc.add_span("tags", Span::on_ref(&split)).unwrap();
    assert_passages_match_scan(&doc);
}

#[test]
fn indexed_passages_follow_edits_cascades_slot_reuse_and_growth() {
    let mut doc = Document::new("test")
        .with_layer("words")
        .with_layer("tags")
        .with_layer("review")
        .with_layer("empty");
    doc.push("0123456789").unwrap();
    let word = doc
        .add_span("words", Span::from(0..2).with_id("word"))
        .unwrap();
    let tag = doc.add_span("tags", Span::on_ref(&word)).unwrap();
    doc.add_span("review", Span::on_ref(&tag)).unwrap();
    assert_passages_match_scan(&doc);
    doc.add_span("words", Span::from(3..3).with_id("point"))
        .unwrap();
    assert_passages_match_scan(&doc);
    doc.replace_span(&word, Span::from([4..6, 8..10])).unwrap();
    assert_passages_match_scan(&doc);
    doc.replace_span(&tag, Span::from(2..4)).unwrap();
    assert_passages_match_scan(&doc);
    doc.remove_span(&tag).unwrap();
    assert_passages_match_scan(&doc);
    doc.add_span("tags", Span::on_ref(&word)).unwrap();
    assert_passages_match_scan(&doc);
    doc.remove_span(&word).unwrap();
    assert_passages_match_scan(&doc);
    doc.push("abcdef").unwrap();
    doc.add_span("words", Span::from(10..16).with_id("word"))
        .unwrap();
    assert_passages_match_scan(&doc);
}
