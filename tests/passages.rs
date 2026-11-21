mod common;

use common::{GENESIS, at, genesis, verse};
use klause::{Error, Passage, Value, select_span, select_spans};

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
        doc.get_passage(select_spans("morph").with_attr("pos", "pronoun")),
        Err(Error::NotOne { found: 0 })
    ));

    let words = doc.get_passages(select_spans("words")).unwrap();
    assert_eq!(words.len(), 17);
    assert_eq!(
        words.iter().take(3).map(Passage::text).collect::<Vec<_>>(),
        ["In", "the", "beginning"]
    );

    // The verses meet, so they come back as one passage.
    let verses = doc.get_passages(select_spans("verses")).unwrap();
    assert_eq!(verses.len(), 1);
    assert_eq!(verses[0].text(), GENESIS);

    let second = doc.get_passage(select_span("verses", "GEN 1:2")).unwrap();
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
