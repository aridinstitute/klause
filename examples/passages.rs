//! Passages — a range of text with every span that touches it, ready to render.
//!
//! API sketch only; this does not compile yet.

use klause::{Document, SpanRef};

fn main() -> Result<(), klause::Error> {
    let doc = Document::new("GEN");
    // … text and spans pushed here …

    let p = doc.get_passage(0..47)?; // a range
    let p = doc.get_passage(SpanRef::new("verses", "3:15"))?; // the text a span covers
    let p = doc.get_passage(..)?; // the whole document
    let ps = doc.get_passages(&selection)?; // Vec<Passage>, in text order

    doc.get_passage([0..3, 42..47]); // Err(NotOne { found: 2 }): two separate ranges

    // Owned, cheap to clone, and Send: cache it, keep it in UI state, send it
    // to another thread. Its text is shared with the document, not copied.
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

    // A passage includes every span that overlaps it, every point inside it or
    // at its edges, every link with an end inside it, and every span on a span
    // already included, at any depth.

    Ok(())
}
