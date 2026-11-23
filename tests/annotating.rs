use klause::{Document, Error, Span, SpanId, SpanRef, Target, Value};

/// "In the beginning", with a word, a morphology tag on it, and a review note on that tag.
fn chain() -> (Document, SpanRef, SpanRef, SpanRef) {
    let mut doc = Document::new("GEN")
        .with_layer("words")
        .with_layer("morph")
        .with_layer("review");
    doc.push("In the beginning").unwrap();
    let word = doc
        .add_span("words", Span::from(7..16).with_id(3_u64))
        .unwrap();
    let tag = doc
        .add_span("morph", Span::on_ref(&word).with_attr("pos", "noun"))
        .unwrap();
    let review = doc
        .add_span(
            "review",
            Span::on("morph", tag.id().clone()).with_attr("status", "ok"),
        )
        .unwrap();
    (doc, word, tag, review)
}

#[test]
fn span_attributes_keep_every_value_of_a_key_until_replaced_or_removed() {
    let span = Span::from(0..3)
        .with_attr("strongs", "H7225")
        .with_attr("pos", "noun")
        .with_attr("strongs", "H7218");

    assert_eq!(span.attr("strongs"), Some(&Value::from("H7225")));
    assert_eq!(
        span.values("strongs").collect::<Vec<_>>(),
        [&Value::from("H7225"), &Value::from("H7218")]
    );
    assert_eq!(
        span.attrs().map(|(key, _)| key).collect::<Vec<_>>(),
        ["strongs", "strongs", "pos"]
    );

    let span = span.set_attr("strongs", "H7218").remove_attr("pos");
    assert_eq!(
        span.values("strongs").collect::<Vec<_>>(),
        [&Value::from("H7218")]
    );
    assert!(span.attr("pos").is_none());
    assert_eq!(span.attrs().count(), 1);

    // An array adds each element as its own value.
    let span = span.with_attr("gloss", ["head", "beginning"]);
    assert_eq!(
        span.values("gloss").collect::<Vec<_>>(),
        [&Value::from("head"), &Value::from("beginning")]
    );
}

#[test]
fn values_convert_from_rust_types_and_compare_numbers_by_value() {
    assert!(matches!(Value::from(3), Value::Int(3)));
    assert!(matches!(Value::from(3_u32), Value::Int(3)));
    assert!(matches!(Value::from(1.5), Value::Float(value) if value == 1.5));
    assert!(matches!(Value::from(true), Value::Bool(true)));
    assert!(matches!(Value::from(String::from("H1")), Value::Str(value) if value == "H1"));

    assert_eq!(Value::from(2), Value::from(2.0));
    assert_ne!(Value::from(2), Value::from(2.5));
    assert_ne!(Value::from(1), Value::from(true));
    assert_ne!(Value::from(2), Value::from("2"));
}

#[test]
fn text_ranges_are_sorted_and_merged_when_they_overlap_or_meet() {
    let mut doc = Document::new("GEN").with_layer("phrases");
    doc.push("In the beginning God").unwrap();

    let phrase = doc
        .add_span("phrases", Span::from([17..20, 0..2, 2..3, 7..10, 8..16]))
        .unwrap();

    assert_eq!(doc.ranges(&phrase).unwrap(), [0..3, 7..16, 17..20]);
    assert!(matches!(
        doc.span(&phrase).unwrap().target(),
        Target::Text(ranges) if ranges == &[0..3, 7..16, 17..20]
    ));
}

#[test]
fn spans_that_target_spans_follow_their_targets_through_a_chain() {
    let (mut doc, word, tag, review) = chain();

    assert!(matches!(
        doc.span(&tag).unwrap().target(),
        Target::Span(target) if target == &word
    ));
    assert_eq!(doc.ranges(&tag).unwrap(), [7..16]);
    assert_eq!(doc.ranges(&review).unwrap(), [7..16]);

    doc.replace_span(&word, Span::from(3..6)).unwrap();
    assert_eq!(doc.ranges(&word).unwrap(), [3..6]);
    assert_eq!(doc.ranges(&tag).unwrap(), [3..6]);
    assert_eq!(doc.ranges(&review).unwrap(), [3..6]);
}

#[test]
fn missing_ids_follow_the_largest_integer_id_in_the_layer() {
    let mut doc = Document::new("GEN")
        .with_layer("verses")
        .with_layer("words");
    doc.push("In the beginning").unwrap();

    // String IDs don't count.
    doc.add_span("verses", Span::from(0..16).with_id("GEN 1:1"))
        .unwrap();
    let first = doc.add_span("verses", Span::from(0..2)).unwrap();
    assert_eq!(first.id(), &SpanId::Int(1));

    // Not the count of spans, and not the gap after 2.
    doc.add_span("words", Span::from(0..2).with_id(5_u64))
        .unwrap();
    doc.add_span("words", Span::from(3..6).with_id(2_u64))
        .unwrap();
    let next = doc.add_span("words", Span::from(7..16)).unwrap();
    assert_eq!(next.layer(), "words");
    assert_eq!(next.id(), &SpanId::Int(6));
}

#[test]
fn add_span_requires_an_existing_layer_and_target() {
    let (mut doc, ..) = chain();

    assert!(matches!(
        doc.add_span("notes", Span::from(0..2)),
        Err(Error::UnknownLayer(id)) if id == "notes"
    ));
    assert!(matches!(
        doc.add_span("morph", Span::on("words", 99_u64)),
        Err(Error::UnknownSpan(target)) if target == SpanRef::new("words", 99_u64)
    ));
    assert_eq!(doc.layer("morph").unwrap().spans().count(), 1);
}

#[test]
fn replace_span_keeps_the_id_and_rejects_cycles() {
    let (mut doc, word, tag, review) = chain();

    doc.replace_span(&tag, Span::on_ref(&word).with_attr("pos", "verb"))
        .unwrap();
    assert_eq!(
        doc.span(&tag).unwrap().attr("pos"),
        Some(&Value::from("verb"))
    );
    assert_eq!(doc.span(&tag).unwrap().id(), Some(tag.id()));

    // Directly onto itself, and through the review that targets it.
    assert!(matches!(
        doc.replace_span(&tag, Span::on_ref(&tag)),
        Err(Error::Cycle(reference)) if reference == tag
    ));
    assert!(matches!(
        doc.replace_span(&tag, Span::on_ref(&review)),
        Err(Error::Cycle(reference)) if reference == tag
    ));
    assert!(matches!(
        doc.replace_span(&tag, Span::on("words", 99_u64)),
        Err(Error::UnknownSpan(_))
    ));
    assert!(matches!(
        doc.replace_span(&SpanRef::new("morph", 99_u64), Span::from(0..2)),
        Err(Error::UnknownSpan(reference)) if reference == SpanRef::new("morph", 99_u64)
    ));

    // Failed replacements leave the chain as it was.
    assert_eq!(doc.ranges(&review).unwrap(), [7..16]);
    assert_eq!(
        doc.span(&tag).unwrap().attr("pos"),
        Some(&Value::from("verb"))
    );
}

#[test]
fn remove_span_cascades_to_spans_that_target_it_in_every_layer() {
    let (mut doc, word, tag, review) = chain();
    let other = doc
        .add_span("words", Span::from(0..2).with_id(1_u64))
        .unwrap();
    let other_tag = doc.add_span("morph", Span::on_ref(&other)).unwrap();

    doc.remove_span(&word).unwrap();

    assert!(doc.span(&word).is_none());
    assert!(doc.span(&tag).is_none());
    assert!(doc.span(&review).is_none());
    assert!(doc.span(&other).is_some());
    assert!(doc.span(&other_tag).is_some());
    assert_eq!(doc.layer("review").unwrap().spans().count(), 0);
    assert!(matches!(
        doc.remove_span(&word),
        Err(Error::UnknownSpan(reference)) if reference == word
    ));
}

fn endpoints() -> (Document, SpanRef, SpanRef) {
    let mut doc = Document::new("run")
        .with_layer("words")
        .with_layer("highlights");
    doc.push("one two  three, four five six").unwrap();
    let first = doc
        .add_span("words", Span::from(9..14).with_id(300_u64))
        .unwrap();
    let last = doc
        .add_span("words", Span::from(21..25).with_id(5_u64))
        .unwrap();
    (doc, first, last)
}

#[test]
fn between_uses_text_order_and_matches_either_endpoint() {
    use klause::spans_in;
    let (mut doc, first, last) = endpoints();
    let highlight = doc
        .add_span("highlights", Span::between(first.clone(), last.clone()))
        .unwrap();
    assert_eq!(doc.ranges(&highlight).unwrap(), &[9..25]);
    assert!(matches!(doc.span(&highlight).unwrap().target(),
        Target::Between(a, b) if a == &first && b == &last));
    for id in [300_u64, 5] {
        let filter = spans_in("highlights").targeting(spans_in("words").with_id(id));
        assert_eq!(doc.get_selection(filter).unwrap().ranges(), &[9..25]);
    }
    assert_eq!(
        doc.get_passage(highlight).unwrap().text(),
        "three, four five"
    );
    assert_eq!(
        doc.find()
            .with_text("four")
            .get_spans("highlights")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn between_rejects_invalid_endpoints() {
    let (mut doc, first, last) = endpoints();
    assert!(matches!(
        doc.add_span("highlights", Span::between(last, first.clone())),
        Err(Error::Reversed)
    ));
    let overlap = doc.add_span("words", Span::from(12..16)).unwrap();
    assert!(matches!(
        doc.add_span("highlights", Span::between(first.clone(), overlap)),
        Err(Error::Reversed)
    ));
    let other = doc.add_span("highlights", Span::from(21..25)).unwrap();
    assert!(matches!(
        doc.add_span("highlights", Span::between(first, other)),
        Err(Error::DifferentLayers)
    ));
}

#[test]
fn between_updates_from_both_endpoints_and_cascades_through_a_diamond() {
    let (mut doc, first, last) = endpoints();
    let start = doc.add_span("words", Span::from(4..7)).unwrap();
    let highlight = doc
        .add_span("highlights", Span::between(first.clone(), last.clone()))
        .unwrap();
    let alias = doc
        .add_span("highlights", Span::on_ref(&highlight))
        .unwrap();

    doc.replace_span(&first, Span::on_ref(&start)).unwrap();
    doc.replace_span(&last, Span::from([16..20, 21..25]))
        .unwrap();
    assert_eq!(doc.ranges(&alias).unwrap(), &[4..25]);

    assert!(matches!(
        doc.replace_span(&last, Span::from(0..3)),
        Err(Error::Reversed)
    ));
    assert_eq!(doc.ranges(&last).unwrap(), &[16..20, 21..25]);
    assert_eq!(doc.ranges(&alias).unwrap(), &[4..25]);

    // Both endpoints now depend on the same span. Point endpoints can coincide.
    doc.replace_span(&start, Span::from(4..4)).unwrap();
    doc.replace_span(&last, Span::on_ref(&start)).unwrap();
    doc.replace_span(&start, Span::from(7..7)).unwrap();
    assert_eq!(doc.ranges(&alias).unwrap(), &[7..7]);
    doc.remove_span(&start).unwrap();
    assert!(doc.span(&highlight).is_none());
    assert!(doc.span(&alias).is_none());
}

#[test]
fn between_removing_one_endpoint_detaches_the_other() {
    let (mut doc, first, last) = endpoints();
    let highlight = doc
        .add_span("highlights", Span::between(first.clone(), last.clone()))
        .unwrap();
    doc.remove_span(&last).unwrap();
    assert!(doc.span(&highlight).is_none());
    let new_span = doc.add_span("highlights", Span::from(0..3)).unwrap();
    doc.remove_span(&first).unwrap();
    assert!(doc.span(&new_span).is_some());
}

#[test]
fn between_cycle_through_second_endpoint_is_rejected() {
    let (mut doc, first, last) = endpoints();
    let between = doc
        .add_span("highlights", Span::between(first, last.clone()))
        .unwrap();
    assert!(
        matches!(doc.replace_span(&last, Span::on_ref(&between)), Err(Error::Cycle(r)) if r == last)
    );
}
