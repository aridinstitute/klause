//! End-to-end checks against all 66 books of the Berean Standard Bible USJ fixture.

#[path = "common/bsb_usj.rs"]
mod bsb_usj;

use klause::{SpanId, Value, spans_in};

#[test]
fn loads_and_filters_the_full_bible() {
    let bible = bsb_usj::load().unwrap();
    let count = |layer| bible.layer(layer).unwrap().spans().count();
    eprintln!(
        "{} bytes, {} chapters, {} verses, {} words, {} notes",
        bible.len(),
        count("chapters"),
        count("verses"),
        count("words"),
        count("notes"),
    );
    assert_eq!(count("books"), 66);
    assert_eq!(count("chapters"), 1189);
    assert_eq!(count("verses"), 31_086);
    assert_eq!(count("words"), 730_467);
    assert_eq!(count("notes"), 4_854);
    assert!(
        bible
            .text()
            .starts_with("In the beginning God created the heavens and the earth.")
    );
    assert!(bible.text().ends_with("Amen."));

    let verse = |id| {
        bible
            .find()
            .in_span("verses", id)
            .with_layers(["structure", "notes"])
            .get_passage()
            .unwrap()
    };
    let creation = verse("GEN 1:1");
    assert_eq!(
        creation.text(),
        "In the beginning God created the heavens and the earth."
    );

    assert!(verse("GEN 1:5").text().contains("\nAnd there was evening"));
    assert_eq!(
        verse("PSA 3:1").text(),
        "A Psalm of David, when he fled from his son Absalom.\nO LORD, how my foes have increased!\nHow many rise up against me!"
    );

    // The note contains an inline reference; neither its label nor its body is scripture text.
    let light = verse("GEN 1:3");
    assert_eq!(
        light.text(),
        "And God said, “Let there be light,” and there was light."
    );
    let note = light.spans_in("notes").next().unwrap();
    assert_eq!(note.attr("to"), Some(&Value::from("2CO 4:6")));
    assert_eq!(
        note.attr("body"),
        Some(&Value::from("Cited in 2 Corinthians 4:6"))
    );
    let at = "And God said, “Let there be light,”".len() as u32;
    assert_eq!(note.local(), [at..at]);
    assert_eq!(
        bible
            .find()
            .in_span("chapters", "GEN 1")
            .count_spans(spans_in("notes").with_attr("to", "2CO 4:6"))
            .unwrap(),
        1
    );

    // John 3:17 starts inside a words-of-Jesus character node spanning several verses.
    let john = verse("JHN 3:17");
    assert_eq!(
        john.text(),
        "For God did not send His Son into the world to condemn the world, but to save the world through Him."
    );
    assert!(
        john.spans_in("structure")
            .any(|span| span.attr("marker") == Some(&Value::from("wj")))
    );

    let john_one = bible
        .find()
        .count_spans(
            spans_in("verses")
                .with_attr("book", "JHN")
                .with_attr("chapter", 1),
        )
        .unwrap();
    assert_eq!(john_one, 51);

    let god_in_first_verse = bible
        .find()
        .in_span("verses", "GEN 1:1")
        .get_spans(spans_in("words").with_attr("token", "god"))
        .unwrap();
    assert_eq!(god_in_first_verse.len(), 1);

    let matches = bible
        .find()
        .in_span("books", "JHN")
        .with_text_folded("for god so loved the world")
        .get_spans("verses")
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id(), Some(&SpanId::from("JHN 3:16")));
}
