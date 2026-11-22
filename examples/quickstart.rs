//! Create a document, annotate its text, and query it.
use klause::{Document, spans_in};

fn main() -> Result<(), klause::Error> {
    let mut doc = Document::new("greeting").with_layer("words");
    doc.push("Hello, ")?;
    doc.push_span("words", "world", |span| span.with_attr("pos", "noun"))?;
    doc.push("!")?;

    let passage = doc
        .find()
        .within(spans_in("words").with_attr("pos", "noun"))
        .get_passage()?;
    assert_eq!(passage.text(), "world");
    Ok(())
}
