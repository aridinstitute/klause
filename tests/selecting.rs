mod common;

use common::{GENESIS, at, genesis, verse};
use klause::{
    Document, Selection, Span, SpanFilter, any_of, select_all_spans, select_span, select_spans,
};

/// Two analyses of the same word, a second word, an alternative layer, and reviews of the analyses.
fn analyses() -> Document {
    let mut doc = Document::new("analyses")
        .with_layer("morph")
        .with_layer("alt")
        .with_layer("review");
    doc.push("aaa bbb ccc").unwrap();
    doc.add_span(
        "morph",
        Span::from(0..3)
            .with_id("a")
            .with_attr("pos", "noun")
            .with_attr("strongs", ["H1", "H2"]),
    )
    .unwrap();
    doc.add_span(
        "morph",
        Span::from(0..3)
            .with_id("b")
            .with_attr("pos", "verb")
            .with_attr("strongs", "H3"),
    )
    .unwrap();
    doc.add_span(
        "morph",
        Span::from(4..7).with_id("c").with_attr("pos", "noun"),
    )
    .unwrap();
    doc.add_span("alt", Span::from(8..11).with_attr("pos", "noun"))
        .unwrap();
    doc.add_span("review", Span::on("morph", "b")).unwrap();
    doc.add_span("review", Span::on("morph", "c")).unwrap();
    doc
}

fn count(doc: &Document, filter: SpanFilter) -> usize {
    doc.find().count_spans(filter).unwrap()
}

#[test]
fn every_condition_of_a_filter_must_hold_on_the_same_span() {
    let doc = analyses();

    // "a" is a noun and "b" has H3, on the same text, but no one span has both.
    let filter = select_spans("morph")
        .with_attr("pos", "noun")
        .with_attr("strongs", "H3");
    assert_eq!(count(&doc, filter.clone()), 0);
    assert!(doc.get_selection(filter).unwrap().is_empty());

    assert_eq!(
        count(
            &doc,
            select_spans("morph")
                .with_attr("pos", "noun")
                .with_attr("strongs", "H2")
        ),
        1
    );
}

#[test]
fn attribute_conditions_look_at_every_value_of_a_key() {
    let doc = analyses();

    // "a" matches on its second value.
    assert_eq!(
        count(&doc, select_spans("morph").with_attr("strongs", "H2")),
        1
    );
    // "a" has H1, but also H2, so it is excluded; "c" has no strongs at all, so it is kept.
    assert_eq!(
        count(&doc, select_spans("morph").without_attr("strongs", "H2")),
        2
    );
    assert_eq!(
        count(
            &doc,
            select_spans("morph").with_attr("strongs", any_of(["H3", "H9"]))
        ),
        1
    );
    assert_eq!(count(&doc, select_spans("morph").with_key("strongs")), 2);
    assert_eq!(count(&doc, select_spans("morph").without_key("strongs")), 1);
}

#[test]
fn numeric_conditions_compare_numbers_by_value() {
    let doc = genesis();
    let verses = |filter: SpanFilter| doc.get_selection(filter).unwrap().ranges().to_vec();

    assert_eq!(
        verses(select_spans("verses").with_attr("number", 2.0)),
        [verse(2)]
    );
    assert_eq!(
        verses(select_spans("verses").with_attr("number", 2..)),
        [verse(2)]
    );
    assert_eq!(
        verses(select_spans("verses").with_attr("number", 1..2)),
        [verse(1)]
    );
    assert_eq!(
        verses(select_spans("verses").with_attr("number", 0.5..1.5)),
        [verse(1)]
    );
    assert_eq!(
        verses(select_spans("verses").with_attr("number", any_of([2, 3]))),
        [verse(2)]
    );
}

#[test]
fn selectors_resolve_to_the_ranges_of_the_spans_they_name() {
    let doc = genesis();

    assert_eq!(
        doc.get_selection(select_span("verses", "GEN 1:2"))
            .unwrap()
            .ranges(),
        [verse(2)]
    );
    assert_eq!(
        doc.get_selection(select_spans("verses").with_id("GEN 1:1"))
            .unwrap()
            .ranges(),
        [verse(1)]
    );
    // Morphology targets words, so it selects the words' text.
    assert_eq!(
        doc.get_selection(select_spans("morph").with_attr("pos", "verb"))
            .unwrap()
            .ranges(),
        [at("created", 0), at("was", 0)]
    );
    // Consecutive verses meet and merge into one range.
    assert_eq!(
        doc.get_selection(select_spans("verses")).unwrap().ranges(),
        [0..GENESIS.len() as u32]
    );
}

#[test]
fn filters_can_span_every_layer_or_match_through_targets() {
    let doc = analyses();

    assert_eq!(
        doc.get_selection(select_all_spans().with_attr("pos", "noun"))
            .unwrap()
            .ranges(),
        [0..3, 4..7, 8..11]
    );
    assert_eq!(
        count(
            &doc,
            select_spans("review").targeting(select_spans("morph").with_attr("pos", "verb"))
        ),
        1
    );
    assert_eq!(
        doc.get_selection(
            select_spans("review").targeting(select_spans("morph").with_attr("pos", "noun"))
        )
        .unwrap()
        .ranges(),
        [4..7]
    );
}

#[test]
fn selections_merge_ranges_that_overlap_or_meet_and_keep_only_isolated_points() {
    let doc = genesis();

    let selection = doc
        .get_selection(vec![20..25, 0..3, 3..10, 30..30, 22..22, 25..25, 0..0])
        .unwrap();

    // 22 and 25 lie inside or at the end of 20..25, and 0 at the start of 0..10.
    assert_eq!(selection.ranges(), [0..10, 20..25, 30..30]);
    assert!(selection.contains(0));
    assert!(selection.contains(9));
    assert!(!selection.contains(10));
    assert!(!selection.is_empty());
    assert_eq!(doc.get_selection(selection.clone()).unwrap(), selection);
}

#[test]
fn set_operations_keep_points_that_lie_within_the_other_side() {
    let doc = genesis();
    let get =
        |ranges: Vec<std::ops::Range<u32>>| -> Selection { doc.get_selection(ranges).unwrap() };

    let a = get(vec![0..10, 20..30]);
    let b = get(vec![5..25, 40..40]);
    let points = get(vec![10..10, 15..15, 30..30]);

    assert_eq!(a.union(&b).ranges(), [0..30, 40..40]);
    assert_eq!(a.intersect(&b).ranges(), [5..10, 20..25]);
    assert_eq!(a.subtract(&b).ranges(), [0..5, 25..30]);
    assert_eq!(get(vec![5..25]).subtract(&a).ranges(), [10..20]);

    // Points at a range's end count as within it; 15 lies in the gap.
    assert_eq!(a.intersect(&points).ranges(), [10..10, 30..30]);
    assert_eq!(points.intersect(&a).ranges(), [10..10, 30..30]);
    assert!(b.intersect(&get(vec![30..35])).is_empty());
}
