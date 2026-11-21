//! The "Example: a Bible" walkthrough from DESIGN.md, end to end.

mod common;

use common::Highlights;
use klause::{Document, Layer, Span, SpanId, Value, select_span, select_spans};

const VERSE: &str = "And I will put enmity between you and the woman, and between your seed \
                     and her seed. He will crush your head, and you will strike his heel.”";

/// Genesis 3:15, built as DESIGN.md builds it.
fn bible() -> Document {
    let mut bible = Document::new("BSB")
        .with_attr("title", "Berean Standard Bible")
        .with_layer("books")
        .with_layer("chapters")
        .with_layer("verses")
        .with_layer("words")
        .with_layer("structure")
        .with_layer("morph")
        .with_layer("notes")
        .with_layer("xrefs");

    let lines: [(i64, &[(&str, &str)]); 4] = [
        (
            1,
            &[
                ("And", " "),
                ("I", " "),
                ("will", " "),
                ("put", " "),
                ("enmity", " "),
                ("between", " "),
                ("you", " "),
                ("and", " "),
                ("the", " "),
                ("woman", ", "),
            ],
        ),
        (
            2,
            &[
                ("and", " "),
                ("between", " "),
                ("your", " "),
                ("seed", " "),
                ("and", " "),
                ("her", " "),
                ("seed", ". "),
            ],
        ),
        (
            1,
            &[
                ("He", " "),
                ("will", " "),
                ("crush", " "),
                ("your", " "),
                ("head", ", "),
            ],
        ),
        (
            2,
            &[
                ("and", " "),
                ("you", " "),
                ("will", " "),
                ("strike", " "),
                ("his", " "),
                ("heel", ".”"),
            ],
        ),
    ];

    let verse_start = bible.len();
    let mut n: u64 = 0;
    for (indent, words) in lines {
        let line_start = bible.len();
        let mut line_end = line_start;
        for &(word, after) in words {
            n += 1;
            line_end = bible
                .push_span("words", word, |s| s.with_id(1_003_015_000 + n))
                .unwrap()
                .end;
            bible.push(after).unwrap();
        }
        bible
            .add_span(
                "structure",
                Span::from(line_start..line_end)
                    .with_attr("type", "line")
                    .with_attr("indent", indent),
            )
            .unwrap();
    }
    bible
        .add_span(
            "verses",
            Span::from(verse_start..bible.len()).with_id("GEN 3:15"),
        )
        .unwrap();

    bible
        .add_span(
            "morph",
            Span::on("words", 1_003_015_005_u64)
                .with_attr("pos", "noun")
                .with_attr("strongs", "H342"),
        )
        .unwrap();
    bible
        .add_span(
            "notes",
            Span::from(bible.len()..bible.len()).with_attr(
                "body",
                "Or He will bruise your head, and you will bruise his heel",
            ),
        )
        .unwrap();
    bible
        .add_span(
            "xrefs",
            Span::on("verses", "GEN 3:15").with_attr("to", "ROM 16:20"),
        )
        .unwrap();
    bible
}

#[test]
fn reading_a_verse_for_display() {
    let bible = bible();

    let p = bible
        .get_passage(select_span("verses", "GEN 3:15"))
        .unwrap();
    assert_eq!(p.range(), 0..142);
    assert_eq!(p.text(), VERSE);

    let lines: Vec<_> = p
        .spans_in("structure")
        .map(|line| (line.text()[0].to_owned(), line.attr("indent").cloned()))
        .collect();
    assert_eq!(
        lines,
        [
            (
                "And I will put enmity between you and the woman".to_owned(),
                Some(Value::from(1))
            ),
            (
                "and between your seed and her seed".to_owned(),
                Some(Value::from(2))
            ),
            ("He will crush your head".to_owned(), Some(Value::from(1))),
            (
                "and you will strike his heel".to_owned(),
                Some(Value::from(2))
            ),
        ]
    );

    let notes: Vec<_> = p.spans_in("notes").collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].local(), [142..142]);

    let xrefs: Vec<_> = p.spans_in("xrefs").collect();
    assert_eq!(xrefs.len(), 1);
    assert_eq!(xrefs[0].attr("to"), Some(&Value::from("ROM 16:20")));

    let enmity = p.spans_in("morph").next().unwrap();
    assert_eq!(enmity.text(), ["enmity"]);

    let plain = bible
        .find()
        .in_span("verses", "GEN 3:15")
        .with_layers(["structure"])
        .get_passage()
        .unwrap();
    assert_eq!(plain.text(), VERSE);
    assert!(plain.spans().all(|span| span.layer() == "structure"));
    assert_eq!(plain.spans().count(), 4);
}

#[test]
fn searching_by_word_and_by_reverse_cross_reference() {
    let bible = bible();
    let verse_ids = |spans: Vec<Span>| -> Vec<SpanId> {
        spans
            .iter()
            .map(|span| span.id().unwrap().clone())
            .collect()
    };

    assert_eq!(
        verse_ids(
            bible
                .find()
                .within(select_spans("morph").with_attr("strongs", "H342"))
                .get_spans("verses")
                .unwrap()
        ),
        [SpanId::from("GEN 3:15")]
    );
    assert_eq!(
        verse_ids(
            bible
                .find()
                .within(select_spans("xrefs").with_attr("to", "ROM 16:20"))
                .get_spans("verses")
                .unwrap()
        ),
        [SpanId::from("GEN 3:15")]
    );
    assert!(
        bible
            .find()
            .within(select_spans("xrefs").with_attr("to", "REV 12:17"))
            .get_spans("verses")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn saving_a_highlight_from_a_selection_on_screen() {
    let mut bible = bible();
    let db = Highlights::default();
    bible
        .add_layer(Layer::source("highlights", db.clone()))
        .unwrap();

    // The user selects "ity betw" on screen, in passage-local positions.
    let p = bible
        .get_passage(select_span("verses", "GEN 3:15"))
        .unwrap();
    let local = 18..26;
    assert_eq!(&p.text()[18..26], "ity betw");
    let start = p.range().start;
    let whole_words = bible
        .find()
        .within(local.start + start..local.end + start)
        .expand_to("words")
        .get_selection()
        .unwrap();
    assert_eq!(whole_words.ranges(), [15..21, 22..29]);
    db.save(
        Span::from(whole_words.ranges().to_vec())
            .with_id(1_u64)
            .with_attr("color", "yellow"),
    );

    let p = bible
        .get_passage(select_span("verses", "GEN 3:15"))
        .unwrap();
    let highlights: Vec<_> = p.spans_in("highlights").collect();
    assert_eq!(highlights.len(), 1);
    assert_eq!(highlights[0].text(), ["enmity", "between"]);

    let highlighted = bible
        .find()
        .in_layer("highlights")
        .expand_to("verses")
        .get_passages()
        .unwrap();
    assert_eq!(highlighted.len(), 1);
    assert_eq!(highlighted[0].text(), VERSE);
}
