//! Spans — the one annotation type: a target, attributes, and an ID, in a layer.
//!
//! API sketch only; this does not compile yet.

use klause::{Document, Span, SpanRef, Target};

fn main() -> Result<(), klause::Error> {
    let mut doc = Document::new("KAI 181");
    // … text pushed here …

    // What a span can point at.
    let range = Span::from(0..3); // some text
    let multi = Span::from([61..70, 75..83]); // several ranges, read as one unit
    let point = Span::from(48..48); // a point between two characters
    let on_span = Span::on(SpanRef::new("words", "1.3")); // a span about a span
    let link = Span::link(0..4, 7..12); // from -> to, within the document
    let link = Span::link(SpanRef::new("words", "1.1"), SpanRef::new("words", "1.2"));

    // Attributes: repeating a key adds another value.
    let s = Span::from(15..17).with_attr("pos", "noun").with_attr("lemma", "bn");
    s.attr("lemma"); // Some(&Value::Str("bn"))

    let s = s.with_attr("lemma", "bnh"); // a second candidate lemma
    s.values("lemma").count(); // 2, whether there is one value or many

    let s = s.set_attr("lemma", "bn"); // replaces every value of the key
    let s = s.without_attr("lemma");
    s.attrs(); // (key, value) pairs, in the order they were first added

    // IDs are unique within a layer and document; add_span assigns a ULID to a
    // span that has none.
    let r = doc.add_span("morph", Span::on(SpanRef::new("words", "1.3")).with_attr("pos", "noun"))?;
    r.layer(); // "morph"
    r.id(); // SpanId::Str("01JD3Q8Z5K7M2N4P6R8T0V2X4Y")

    // Replace keeps the ID; removing a span also removes every span on it.
    doc.replace_span(&r, Span::on(&r).with_attr("pos", "noun").with_attr("state", "cstr"))?;
    doc.remove_span(&r)?;

    // Inspecting the target.
    match s.target() {
        Target::Text(loc) => {
            loc.ranges(); // [61..70, 75..83]
            loc.bounds(); // 61..83
        }
        Target::Span(r) => {
            doc.span(r)?;
        }
        Target::Link { from, to } => {
            doc.get_passage(to)?; // either end works as a selection
        }
    }

    // The text a span covers, following spans on spans.
    doc.resolve(&s);

    Ok(())
}
