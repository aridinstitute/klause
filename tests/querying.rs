mod common;

use std::ops::Range;

use common::{GENESIS, at, genesis, verse};
use klause::{
    Distance, Document, Error, Span, SpanId, Target, Value, all_spans, span_id, spans_in,
};

fn hits(query: klause::Query<'_>) -> Vec<Range<u32>> {
    query.get_selection().unwrap().ranges().to_vec()
}

fn nouns() -> klause::SpanFilter {
    spans_in("morph").with_attr("pos", "noun")
}

fn verbs() -> klause::SpanFilter {
    spans_in("morph").with_attr("pos", "verb")
}

#[test]
fn a_query_without_steps_hits_the_whole_text() {
    let doc = genesis();

    assert_eq!(hits(doc.find()), [0..GENESIS.len() as u32]);
    assert_eq!(doc.find().count().unwrap(), 1);
    assert_eq!(doc.find().get_passage().unwrap().text(), GENESIS);
}

#[test]
fn within_outside_and_layer_steps_narrow_the_hits() {
    let doc = genesis();

    // The gaps between the words of verse 1, worked out from the text.
    let mut gaps = Vec::new();
    let mut start = None;
    for (index, char) in GENESIS[..verse(1).end as usize].char_indices() {
        match (char.is_alphabetic(), start) {
            (false, None) => start = Some(index as u32),
            (true, Some(gap)) => {
                gaps.push(gap..index as u32);
                start = None;
            }
            _ => {}
        }
    }
    gaps.extend(start.map(|gap| gap..verse(1).end));

    assert_eq!(
        hits(
            doc.find()
                .in_span("verses", "GEN 1:1")
                .outside(spans_in("words"))
        ),
        gaps
    );
    assert_eq!(
        hits(
            doc.find()
                .within(span_id("verses", "GEN 1:2"))
                .in_layer("words")
        )
        .len(),
        7
    );
    // Steps between `expand_to`s give the same hits in any order.
    assert_eq!(
        hits(
            doc.find()
                .outside(nouns())
                .in_span("verses", "GEN 1:2")
                .in_layer("words")
        ),
        hits(
            doc.find()
                .in_layer("words")
                .in_span("verses", "GEN 1:2")
                .outside(nouns())
        ),
    );
}

#[test]
fn with_text_finds_occurrences_wholly_inside_one_hit() {
    let doc = genesis();

    assert_eq!(
        hits(doc.find().with_text("the earth")),
        [at("the earth", 0), at("the earth", 1)]
    );
    // Each word is its own hit, so a phrase across two words is not inside one.
    assert!(hits(doc.find().in_layer("words").with_text("the earth")).is_empty());
    // Case-sensitive: "In" is not "in".
    assert_eq!(hits(doc.find().with_text("in")), [at("in", 0), at("in", 1)]);
    assert_eq!(
        hits(doc.find().in_span("verses", "GEN 1:2").with_text("the")),
        [at("the", 3)]
    );
}

#[test]
fn with_text_folded_ignores_case_and_combining_marks() {
    let mut doc = Document::new("folding");
    doc.push(
        "\u{02BE}\u{0114}l\u{014D}h\u{00EE}m ELOHIM \
         \u{05D0}\u{05B1}\u{05DC}\u{05B9}\u{05D4}\u{05B4}\u{05D9}\u{05DD}",
    )
    .unwrap();

    // The leading ʾ is a letter, not a mark, so it stays outside the match.
    assert_eq!(hits(doc.find().with_text_folded("elohim")), [2..11, 12..18]);
    assert!(hits(doc.find().with_text("elohim")).is_empty());
    assert_eq!(hits(doc.find().with_text_folded("אלהים")), [19..35]);
    assert_eq!(hits(doc.find().with_text_folded("\u{02BE}elohim")), [0..11]);
}

#[test]
fn followed_by_and_preceded_by_keep_hits_with_a_neighbour_within_the_distance() {
    let doc = genesis();

    assert_eq!(
        hits(
            doc.find()
                .within(nouns())
                .followed_by(verbs(), Distance::adjacent())
        ),
        [at("God", 0), at("earth", 1)]
    );
    assert_eq!(
        hits(
            doc.find()
                .within(verbs())
                .preceded_by(nouns(), Distance::adjacent())
        ),
        [at("created", 0), at("was", 0)]
    );
    // Punctuation and whitespace count as adjacent.
    assert_eq!(
        hits(doc.find().within(span_id("words", 10_u64)).followed_by(
            spans_in("morph").with_attr("pos", "adv"),
            Distance::adjacent()
        )),
        [at("earth", 0)]
    );

    // " God " lies between "beginning" and "created": 5 bytes, one word.
    let beginning = || doc.find().within(at("beginning", 0));
    assert!(hits(beginning().followed_by(verbs(), Distance::adjacent())).is_empty());
    assert_eq!(
        hits(beginning().followed_by(verbs(), Distance::bytes(5))),
        [at("beginning", 0)]
    );
    assert!(hits(beginning().followed_by(verbs(), Distance::bytes(4))).is_empty());
    assert_eq!(
        hits(beginning().followed_by(verbs(), Distance::at_most(1, "words"))),
        [at("beginning", 0)]
    );
    assert!(hits(beginning().followed_by(verbs(), Distance::at_most(0, "words"))).is_empty());
    // Nothing that comes before counts.
    assert!(hits(beginning().preceded_by(verbs(), Distance::bytes(50))).is_empty());
}

#[test]
fn expand_to_replaces_the_hits_and_only_later_steps_see_the_new_ones() {
    let doc = genesis();

    // A selection from inside "beginning" to inside "created" becomes those whole words.
    assert_eq!(
        hits(doc.find().within(10..23).expand_to("words")),
        [at("beginning", 0), at("God", 0), at("created", 0)]
    );

    // "heavens" is only in verse 1, so only verse 1's "the"s are found after expanding.
    assert_eq!(
        hits(
            doc.find()
                .with_text("heavens")
                .expand_to("verses")
                .with_text("the")
        ),
        [at("the", 0), at("the", 1), at("the", 2)]
    );
    // Searching first finds a "the" in each verse; the verses then merge into one range.
    assert_eq!(
        hits(doc.find().with_text("the").expand_to("verses")),
        [0..GENESIS.len() as u32]
    );
    assert_eq!(
        hits(
            doc.find()
                .within(span_id("words", 4_u64))
                .expand_to(spans_in("verses"))
        ),
        [verse(1)]
    );
}

#[test]
fn get_spans_returns_spans_touching_the_hits_by_start_then_layer_order() {
    let doc = genesis();

    let spans = doc
        .find()
        .within(at("God", 0))
        .get_spans(all_spans())
        .unwrap();

    assert_eq!(
        spans
            .iter()
            .map(|span| span.id().unwrap().clone())
            .collect::<Vec<_>>(),
        [
            SpanId::from("GEN"),
            SpanId::from("GEN 1"),
            SpanId::from("GEN 1:1"),
            SpanId::from(4_u64),
            SpanId::from(4_u64),
        ]
    );
    assert!(matches!(spans[3].target(), Target::Text(_)));
    assert!(matches!(spans[4].target(), Target::Span(_)));

    let verses = doc
        .find()
        .within(spans_in("morph").with_attr("strongs", "H776"))
        .get_spans("verses")
        .unwrap();
    assert_eq!(verses.len(), 2);
}

#[test]
fn counts_report_hit_ranges_spans_and_values() {
    let doc = genesis();

    assert_eq!(doc.find().within(nouns()).count().unwrap(), 7);
    assert_eq!(
        doc.find()
            .in_span("verses", "GEN 1:1")
            .count_spans("words")
            .unwrap(),
        10
    );
    // Most frequent first; ties in value order.
    assert_eq!(
        doc.find().count_values("morph", "pos").unwrap(),
        [
            (Value::from("noun"), 7),
            (Value::from("art"), 4),
            (Value::from("conj"), 2),
            (Value::from("verb"), 2),
            (Value::from("adv"), 1),
            (Value::from("prep"), 1),
        ]
    );
    assert_eq!(
        doc.find()
            .in_span("verses", "GEN 1:2")
            .count_values("morph", "pos")
            .unwrap(),
        [
            (Value::from("noun"), 3),
            (Value::from("adv"), 1),
            (Value::from("art"), 1),
            (Value::from("conj"), 1),
            (Value::from("verb"), 1),
        ]
    );
}

#[test]
fn count_values_orders_ties_null_then_bools_then_numbers_then_strings() {
    let mut doc = Document::new("values").with_layer("x");
    doc.push("x").unwrap();
    let values = [
        Value::from("a"),
        Value::from(10),
        Value::from(2),
        Value::from("B"),
        Value::from(true),
        Value::from(2.0),
        Value::from(9),
        Value::Null,
        Value::from(1.5),
        Value::from(false),
    ];
    for value in values {
        doc.add_span("x", Span::from(0..1).with_attr("v", value))
            .unwrap();
    }

    let counts = doc.find().count_values("x", "v").unwrap();

    // 2 and 2.0 are one value.
    assert_eq!(
        counts,
        [
            (Value::from(2), 2),
            (Value::Null, 1),
            (Value::from(false), 1),
            (Value::from(true), 1),
            (Value::from(1.5), 1),
            (Value::from(9), 1),
            (Value::from(10), 1),
            (Value::from("B"), 1),
            (Value::from("a"), 1),
        ]
    );
}

#[test]
fn queries_fail_when_a_step_or_verb_names_an_unknown_layer() {
    let doc = genesis();

    assert!(matches!(
        doc.find().in_layer("missing").count(),
        Err(Error::UnknownLayer(id)) if id == "missing"
    ));
    assert!(matches!(
        doc.find().get_spans("missing"),
        Err(Error::UnknownLayer(_))
    ));
    assert!(matches!(
        doc.find().expand_to("missing").within(0..5).get_selection(),
        Err(Error::UnknownLayer(_))
    ));
    assert!(matches!(
        doc.get_selection(spans_in("missing")),
        Err(Error::UnknownLayer(_))
    ));
}
