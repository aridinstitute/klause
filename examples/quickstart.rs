//! The smallest useful flow: push text, annotate it, read it back.
//!
//! API sketch only; this does not compile yet.

use klause::{Document, Span, spans};

fn main() -> Result<(), klause::Error> {
    let mut doc = Document::new("greeting");

    let hello = doc.push_span("words", "Hello", |s| s)?;
    doc.push(", ")?;
    let world = doc.push_span("words", "world", |s| s)?;
    doc.push("!")?;

    doc.add_span("pos", Span::from(hello).with_attr("tag", "interjection"))?;
    doc.add_span("pos", Span::from(world).with_attr("tag", "noun"))?;

    let nouns = doc.find().covered_by(spans("pos").attr("tag", "noun")).get_selection()?;
    assert_eq!(nouns.ranges(), [("greeting", 7..12)]);

    let p = doc.get_passage(..)?;
    assert_eq!(p.text(), "Hello, world!");

    for span in p.spans() {
        span.layer(); // "words", "words", "pos", "pos"
        p.text_of(&span); // ["Hello"], ["world"], ["Hello"], ["world"]
    }

    Ok(())
}
