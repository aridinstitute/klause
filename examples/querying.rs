//! Reading and querying — passages, selections, and the query builder.
//!
//! API sketch only; this does not compile yet.

use klause::{Distance, Document, SpanRef, any_of, any_spans, spans};

fn main() -> Result<(), klause::Error> {
    let doc = Document::new("GEN");
    // … text and spans pushed here …

    // -- Passages ----------------------------------------------------------
    // A range of text with every span that touches it, ready to render.
    let p = doc.get_passage(0..47)?; // a range
    let p = doc.get_passage(SpanRef::new("verses", "3:15"))?; // the text a span covers
    let p = doc.get_passage(..)?; // the whole document
    let ps = doc.get_passages(&selection)?; // Vec<Passage>, in text order

    doc.get_passage([0..3, 42..47]); // Err(NotOne { found: 2 }): two separate ranges

    // Owned, cheap to clone, and Send; its text is shared with the document.
    p.doc(); // "GEN"
    p.range(); // 0..142, in document positions
    p.text(); // "And I will put enmity between you and the woman, … his heel.”"

    for span in p.spans() {
        // every layer, in the order the layers were added
        span.layer(); // "structure"
        p.local(&span); // [0..47]: its ranges inside the passage, relative to p.text()
        p.text_of(&span); // ["And I will put enmity between you and the woman"]
        span.begins_before(&p); // false
        span.ends_after(&p); // false
    }

    p.spans_in("words"); // one layer
    p.layer("highlights")?.error(); // Some(SourceError) if that source failed
    p.snap(5..12, "words"); // Some(6..14): widened to whole words, in document positions

    // -- Selections --------------------------------------------------------
    // A sorted set of ranges: plain data with no borrow of the document.
    let a = doc.get_selection(0..47)?;
    let b = doc.get_selection([42..47, 61..70])?;

    a.union(&b); // each returns a new Selection
    a.intersect(&b);
    a.subtract(&b);

    b.ranges(); // [("GEN", 42..47), ("GEN", 61..70)]
    b.contains("GEN", 44); // true
    b.is_empty();

    // Anything IntoSelection works: ranges, `..`, arrays of ranges, SpanRef,
    // &Span, link ends, and Selection itself.
    doc.get_passage(&b)?;

    // -- Filters -----------------------------------------------------------
    // A filter describes spans. Every condition in one filter must hold on the
    // same span.
    spans("morph").attr("pos", "verb"); // has this value
    spans("morph").attr("pos", any_of(["noun", "pn"])); // has one of these values
    spans("morph").not_attr("stem", "qal"); // doesn't have this value
    spans("morph").has("lemma"); // has the key, any value
    spans("morph").lacks("gloss");
    spans("structure").attr("indent", 2..); // a number in this range
    spans("verses").id("3:15");
    spans("review").on(spans("morph").attr("pos", "verb")); // target span matches a filter
    any_spans().attr("is_parable", true); // a span in any layer

    // -- Steps -------------------------------------------------------------
    // Lines holding a construct noun directly followed by a proper name.
    doc.find()
        .covered_by(spans("morph").attr("pos", "noun").attr("state", "cstr"))
        .followed_by(spans("morph").attr("pos", "pn"), Distance::adjacent())
        .expand_to("lines")
        .get_passages()?;

    // Filter steps combine with AND, in any order; the query runs the cheapest
    // first.
    doc.find()
        .within(0..100) // any IntoSelection
        .not_covered_by(spans("review")) // remove covered text
        .in_layer("words") // covered_by(spans(id))
        .with_attr("pos", "noun") // covered_by(any_spans().attr(k, v))
        .with_text("bn") // exact matches
        .with_text_folded("BN") // ignoring case and accents
        .containing(spans("notes")) // whole hits that contain a match
        .near(spans("verses"), Distance::spans("words", 0..=2))
        .only_layers(["morph", "words"]) // limit passages and any_spans()
        .count()?;

    // Steps that change what the hits are apply exactly where they appear:
    // filters before them see the old hits, filters after them the new ones.
    doc.find()
        .covered_by(spans("review").attr("status", "uncertain"))
        .follow("echoes") // to the `to` ends of links whose `from` touches a hit
        .follow_back("echoes") // and the other direction
        .expand_to("verses")
        .get_selection()?;

    // -- Final verbs -------------------------------------------------------
    // Errors from any step surface here.
    doc.find().get_selection()?; // Selection, cheap
    doc.find().get_passages()?; // Vec<Passage>, touching hits merged
    doc.find().get_passage()?; // Passage, errors unless there is exactly one
    doc.find().get_spans("morph")?; // spans in a layer that touch a hit
    doc.find().count()?; // number of hit ranges
    doc.find().count_in(spans("words"))?; // spans matching f that contain a hit
    doc.find().count_values("morph.lemma")?; // [(Value, usize)], most frequent first

    // OR: any_of covers one key with several values. For anything else, run two
    // queries and union the selections.
    let nouns = doc.find().covered_by(spans("morph").attr("pos", "noun")).get_selection()?;
    let verbs = doc.find().covered_by(spans("morph").attr("pos", "verb")).get_selection()?;
    nouns.union(&verbs);

    Ok(())
}
