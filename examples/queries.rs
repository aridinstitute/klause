//! Queries — a fluent builder: filters choose text, the final verb shapes the result.
//!
//! API sketch only; this does not compile yet.

use klause::{Distance, Document, any_of, any_spans, spans};

fn main() -> Result<(), klause::Error> {
    let doc = Document::new("KAI 181");
    // … text and spans pushed here …

    // Filters describe spans. Every condition in one filter must hold on the
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

    // Lines holding a construct noun directly followed by a proper name.
    doc.find()
        .covered_by(spans("morph").attr("pos", "noun").attr("state", "cstr"))
        .followed_by(spans("morph").attr("pos", "pn"), Distance::adjacent())
        .expand_to("lines")
        .get_passages()?;

    // Steps that narrow or widen the hits. Filter steps combine with AND and
    // may be written in any order.
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

    // Final verbs. Errors from any step surface here.
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
