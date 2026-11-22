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
fn folded_search_keeps_overlaps_and_never_matches_across_hits() {
    let mut doc = Document::new("folding");
    doc.push("xÁbábA! ab ab").unwrap();

    // Two overlapping matches normalize to their union.
    assert_eq!(hits(doc.find().with_text_folded("aba")), [1..8]);
    // Neither "ab" range contains "abab"; the matcher resets between ranges.
    assert!(hits(doc.find().within([10..12, 13..15]).with_text_folded("abab")).is_empty());
    assert!(hits(doc.find().with_text_folded("\u{0301}")).is_empty());
}

#[test]
fn folded_search_maps_decomposition_matches_and_ignored_marks_to_original_bytes() {
    let mut doc = Document::new("folding");
    doc.push("x각a\u{20dd}B").unwrap();

    // A last-component match still covers the entire Hangul syllable.
    assert_eq!(hits(doc.find().with_text_folded("\u{11a8}")), [1..4]);
    assert_eq!(hits(doc.find().with_text_folded("ab")), [4..9]);
    assert_eq!(hits(doc.find().with_text_folded("a")), [4..5]);
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
fn span_count_proximity_agrees_with_a_scan_for_points_and_multirange_spans() {
    let mut doc = Document::new("proximity")
        .with_layer("neighbours")
        .with_layer("between");
    doc.push("012345678901234567890123456789").unwrap();
    let candidates = [25..28, 8..9, 17..17, 2..4, 8..10];
    for candidate in &candidates {
        doc.add_span("neighbours", Span::from(candidate.clone()))
            .unwrap();
    }
    let intervening = [
        vec![5..6, 7..8, 13..14],
        vec![6..20],
        vec![9..9],
        vec![17..17],
    ];
    for ranges in &intervening {
        doc.add_span("between", Span::from(ranges.clone())).unwrap();
    }
    let input = [0..1, 4..5, 9..9, 11..12, 16..17, 19..20, 29..30];
    for follows in [true, false] {
        for limit in 0..4 {
            let expected: Vec<_> = input
                .iter()
                .filter(|hit| {
                    candidates.iter().any(|candidate| {
                        let gap = if follows && hit.end <= candidate.start {
                            hit.end..candidate.start
                        } else if !follows && candidate.end <= hit.start {
                            candidate.end..hit.start
                        } else {
                            return false;
                        };
                        intervening
                            .iter()
                            .filter(|ranges| {
                                ranges
                                    .iter()
                                    .any(|r| gap.start <= r.start && r.end <= gap.end)
                            })
                            .count()
                            <= limit
                    })
                })
                .cloned()
                .collect();
            let query = doc.find().within(input.clone());
            let distance = Distance::at_most(limit, "between");
            let query = if follows {
                query.followed_by("neighbours", distance)
            } else {
                query.preceded_by("neighbours", distance)
            };
            assert_eq!(hits(query), expected, "follows={follows}, limit={limit}");
        }
    }
}

#[test]
fn whole_document_counts_skip_empty_targets_and_vacant_slots() {
    let mut doc = Document::new("counts")
        .with_layer("words")
        .with_layer("morph");
    doc.push("word").unwrap();
    let word = doc.add_span("words", Span::from(0..4)).unwrap();
    doc.add_span("morph", Span::on("words", word.id().clone()))
        .unwrap();
    assert_eq!(doc.find().count_spans(all_spans()).unwrap(), 2);

    doc.replace_span(&word, Span::from(Vec::<Range<u32>>::new()))
        .unwrap();
    assert_eq!(doc.find().count_spans(all_spans()).unwrap(), 0);
    doc.replace_span(&word, Span::from(4..4)).unwrap();
    assert_eq!(doc.find().count_spans(all_spans()).unwrap(), 2);
    doc.remove_span(&word).unwrap();
    assert_eq!(doc.find().count_spans(all_spans()).unwrap(), 0);
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
fn count_values_preserves_exact_numeric_equality_and_the_first_representation() {
    let mut doc = Document::new("values").with_layer("x");
    doc.push("x").unwrap();
    let exact = 1_i64 << 53;
    let nan = f64::NAN;
    for values in [
        vec![Value::Float(-0.0), Value::Int(0), Value::Float(0.0)],
        // Promote the accumulator before seeing the next representation of zero.
        ["a", "b", "c", "d", "e", "f", "g", "h"]
            .map(Value::from)
            .to_vec(),
        vec![Value::Int(0)],
        vec![Value::Int(exact + 1)],
        vec![Value::Float(exact as f64), Value::Int(exact)],
        vec![Value::Int(exact)],
        vec![Value::Int(i64::MAX), Value::Float(i64::MAX as f64)],
        vec![Value::Float(nan), Value::Float(nan)],
    ] {
        doc.add_span("x", Span::from(0..1).with_attr("v", values))
            .unwrap();
    }
    let counts = doc.find().count_values("x", "v").unwrap();
    assert_eq!(counts.len(), 15);
    assert!(matches!(counts[0], (Value::Float(n), 2) if n.to_bits() == (-0.0_f64).to_bits()));
    assert!(matches!(counts[1], (Value::Float(n), 2) if n == exact as f64));
    assert!(counts.contains(&(Value::Int(exact + 1), 1)));
    assert!(counts.contains(&(Value::Int(i64::MAX), 1)));
    assert!(counts.contains(&(Value::Float(i64::MAX as f64), 1)));
    assert_eq!(
        counts
            .iter()
            .filter(|(value, count)| {
                *count == 1 && matches!(value, Value::Float(n) if n.is_nan())
            })
            .count(),
        2
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

#[test]
fn selections_limit_literal_and_folded_searches_regardless_of_chain_order() {
    let doc = genesis();
    for folded in [false, true] {
        let phrase = if folded {
            doc.find().with_text_folded("THE EARTH")
        } else {
            doc.find().with_text("the earth")
        };
        assert!(hits(phrase.in_layer("words")).is_empty());
        let word = if folded {
            doc.find().with_text_folded("THE")
        } else {
            doc.find().with_text("the")
        };
        assert_eq!(hits(word.in_span("verses", "GEN 1:2")), [at("the", 3)]);
    }
    assert!(
        hits(
            doc.find()
                .with_text("the earth")
                .outside(at("earth", 0))
                .in_span("verses", "GEN 1:1")
        )
        .is_empty()
    );
}

#[test]
fn query_planning_keeps_expansions_as_boundaries_and_text_steps_in_order() {
    let doc = genesis();
    assert_eq!(
        hits(
            doc.find()
                .with_text("God")
                .in_span("verses", "GEN 1:1")
                .expand_to("verses")
                .with_text("the")
        ),
        [at("the", 0), at("the", 1), at("the", 2)]
    );
    assert!(
        hits(
            doc.find()
                .with_text("God")
                .in_span("verses", "GEN 1:1")
                .expand_to("verses")
                .with_text("the")
                .in_span("verses", "GEN 1:2")
        )
        .is_empty()
    );
    assert_eq!(
        hits(doc.find().with_text("the earth").with_text("earth")),
        [at("earth", 0), at("earth", 1)]
    );
    assert!(hits(doc.find().with_text("earth").with_text("the earth")).is_empty());
}

#[test]
fn proximity_filters_run_after_selections_and_text_searches() {
    let doc = genesis();
    assert_eq!(
        hits(
            doc.find()
                .followed_by(verbs(), Distance::adjacent())
                .with_text("God")
                .in_layer("words")
        ),
        [at("God", 0)]
    );
    assert_eq!(
        hits(
            doc.find()
                .preceded_by(nouns(), Distance::bytes(1))
                .with_text("created")
                .in_layer("words")
        ),
        [at("created", 0)]
    );
}

#[test]
fn custom_selectors_are_deferred_and_receive_narrowed_hits_in_cost_order() {
    use klause::{IntoSelection, Selection};
    use std::{cell::RefCell, rc::Rc};

    type Calls = RefCell<Vec<(usize, Vec<Range<u32>>)>>;
    struct Tracked<'a> {
        id: usize,
        cost: usize,
        calls: &'a Calls,
        resolved: Rc<RefCell<usize>>,
    }
    impl IntoSelection for Tracked<'_> {
        fn estimate_cost(&self, _doc: &Document) -> usize {
            self.cost
        }
        fn may_select_points(&self, _doc: &Document) -> bool {
            false
        }
        fn into_selection(self, doc: &Document) -> Result<Selection, Error> {
            *self.resolved.borrow_mut() += 1;
            (1..3).into_selection(doc)
        }
        fn into_selection_within(
            self,
            doc: &Document,
            hits: &Selection,
        ) -> Result<Selection, Error> {
            self.calls
                .borrow_mut()
                .push((self.id, hits.ranges().to_vec()));
            Ok(hits.intersect(&self.into_selection(doc)?))
        }
    }
    let mut doc = Document::new("custom");
    doc.push("abcdef").unwrap();
    let calls = RefCell::new(Vec::new());
    let resolved = Rc::new(RefCell::new(0));
    let query = doc
        .find()
        .within(Tracked {
            id: 1,
            cost: 100,
            calls: &calls,
            resolved: resolved.clone(),
        })
        .within(Tracked {
            id: 2,
            cost: 10,
            calls: &calls,
            resolved: resolved.clone(),
        });
    assert_eq!(*resolved.borrow(), 0);
    assert_eq!(hits(query), [1..3]);
    assert_eq!(*resolved.borrow(), 2);
    assert_eq!(*calls.borrow(), [(2, vec![0..6]), (1, vec![1..3])]);
}

#[test]
fn narrowed_span_filters_agree_with_full_selections_including_points() {
    use klause::IntoSelection;
    let mut doc = Document::new("filter")
        .with_layer("spans")
        .with_layer("tags");
    doc.push("0123456789abcdef").unwrap();
    for start in 0..=16 {
        for end in start..=16 {
            let span = doc
                .add_span(
                    "spans",
                    Span::from(start..end)
                        .with_attr("kind", if start % 2 == 0 { "even" } else { "odd" }),
                )
                .unwrap();
            doc.add_span("tags", Span::on_ref(&span)).unwrap();
        }
    }
    let split = doc
        .add_span(
            "spans",
            Span::from([0..2, 7..9, 14..16]).with_attr("kind", "even"),
        )
        .unwrap();
    doc.add_span("tags", Span::on_ref(&split)).unwrap();
    for filter in [
        spans_in("spans"),
        spans_in("spans").with_attr("kind", "even"),
        spans_in("tags"),
    ] {
        let full = filter.clone().into_selection(&doc).unwrap();
        for start in 0..=16 {
            for end in start..=16 {
                let selection = (start..end).into_selection(&doc).unwrap();
                assert_eq!(
                    hits(doc.find().within(start..end).within(filter.clone())),
                    selection.intersect(&full).ranges(),
                    "filter {filter:?} in {start}..{end}"
                );
            }
        }
    }
}

#[test]
fn point_selection_order_does_not_change_when_an_index_is_warmed() {
    let mut doc = Document::new("points").with_layer("spans");
    doc.push("ab").unwrap();
    doc.add_span("spans", Span::from(1..1).with_attr("kind", "point"))
        .unwrap();
    let query = || {
        doc.find()
            .within(0..1)
            .within(1..2)
            .within(spans_in("spans").with_attr("kind", "point"))
    };
    assert!(hits(query()).is_empty());
    doc.find()
        .within(spans_in("spans").with_attr("kind", "point"))
        .count()
        .unwrap();
    assert!(hits(query()).is_empty());
    assert_eq!(
        hits(doc.find().within(1..1).within(0..1).within(1..2)),
        [1..1]
    );
}

#[test]
fn empty_hits_do_not_hide_invalid_selections_or_distance_layers() {
    let doc = genesis();
    assert!(matches!(
        doc.find()
            .within(Vec::<Range<u32>>::new())
            .in_layer("missing")
            .count(),
        Err(Error::UnknownLayer(_))
    ));
    assert!(matches!(
        doc.find()
            .within(Vec::<Range<u32>>::new())
            .within(0..u32::MAX)
            .count(),
        Err(Error::OutOfRange(_))
    ));
    assert!(matches!(
        doc.find()
            .with_text("absent")
            .followed_by("words", Distance::at_most(1, "missing"))
            .count(),
        Err(Error::UnknownLayer(_))
    ));
}

#[test]
fn position_candidates_preserve_points_absorbed_by_adjacent_ranges() {
    let mut doc = Document::new("boundary").with_layer("spans");
    doc.push("0123456789").unwrap();
    doc.add_span("spans", Span::from(0..1)).unwrap();
    doc.add_span("spans", Span::from(1..1)).unwrap();
    for _ in 0..100 {
        doc.add_span("spans", Span::from(8..9)).unwrap();
    }
    // The point at 1 was absorbed into 0..1 by selection normalization.
    // That text range does not overlap 1..2, so no point should reappear.
    assert!(hits(doc.find().within(1..2).in_layer("spans")).is_empty());
    assert_eq!(hits(doc.find().within(1..1).in_layer("spans")), [1..1]);
}
