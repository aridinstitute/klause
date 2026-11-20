//! Annotating — documents, spans, layers, and source layers: the data model.
//!
//! API sketch only; this does not compile yet.

use std::ops::Range;

use klause::{Document, Layer, Span, SpanFilter, SpanRef, SpanSource, SourceError, Target};

fn main() -> Result<(), klause::Error> {
    // -- Documents ---------------------------------------------------------
    // Text only grows, is normalised to NFC, and positions are byte offsets.
    let mut doc = Document::new("KAI 181")
        .with_attr("language", "Moabite")
        .with_attr("site", "Dhiban")
        .with_attr("date", -840); // years as integers, negative for BCE

    let a = doc.push("ʾnk")?; // 0..4: positions are bytes, and ʾ takes two
    doc.push(" . ")?;
    let b = doc.push("mšʿ")?; // 7..12

    doc.len(); // 12
    doc.text(); // "ʾnk . mšʿ"
    doc.slice(b); // "mšʿ"
    doc.attr("language"); // Some(&Value::Str("Moabite"))

    doc.push("\u{05B7}")?; // Err(SplitsCharacter): may not start with a combining mark

    // push_span adds the text and a span over it in one step; the closure
    // receives the new span.
    doc.push_span("words", "bn", |s| s.with_id("1.3").with_attr("restored", false))?;
    doc.push_span("words", "kmšyt", |s| s)?; // no ID or attributes

    // Spans covering several pushes are added afterwards from recorded positions.
    let start = doc.len();
    // … push the words of the line …
    doc.add_span("lines", Span::from(start..doc.len()).with_id("1"))?;

    // -- Spans -------------------------------------------------------------
    // A span is a target, attributes, and an ID, in one layer.
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

    // IDs are unique within a layer and document; add_span assigns a ULID when
    // a span has none.
    let r = doc.add_span("morph", Span::on(SpanRef::new("words", "1.3")).with_attr("pos", "noun"))?;
    r.layer(); // "morph"
    r.id(); // SpanId::Str("01JD3Q8Z5K7M2N4P6R8T0V2X4Y")

    // Replace keeps the ID; removing a span also removes every span on it.
    doc.replace_span(&r, Span::on(&r).with_attr("pos", "noun").with_attr("state", "cstr"))?;
    doc.remove_span(&r)?;

    // Inspecting a target.
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

    doc.resolve(&s); // the text a span covers, following spans on spans

    // -- Layers ------------------------------------------------------------
    // A named set of spans. Stored layers live in the document; source layers
    // are read live from your own storage.
    doc.add_layer(
        Layer::new("morph")
            .with_name("Morphology")
            .with_attr("creator", "A. Scholar")
            .with_attr("license", "CC-BY-4.0"),
    )?;

    doc.layer("morph")?.name(); // "Morphology"
    doc.layer("morph")?.attr("license"); // Some(&Value::Str("CC-BY-4.0"))
    doc.layers(); // in the order they were added

    // Layer IDs can't contain dots, because filters write "layer.key".
    doc.add_layer(Layer::new("bad.id"))?; // Err(...)

    // Adding a span to a missing stored layer creates it implicitly. Should it?
    doc.add_span("notes", Span::from(0..9))?;

    // -- Source layers -----------------------------------------------------
    // Read live from your own storage; Klause never writes to it, so a change
    // you save shows up in the next passage or query.
    doc.add_layer(Layer::from_source(Highlights { db: Db::open() }))?;

    Ok(())
}

struct Highlights {
    db: Db,
}

impl SpanSource for Highlights {
    fn id(&self) -> &str {
        "highlights"
    }

    fn name(&self) -> &str {
        "Highlights"
    }

    // Every span with text in `range` of document `doc`, including links with
    // an end there.
    fn get_spans(&self, doc: &str, range: Range<u32>) -> Result<Vec<Span>, SourceError> {
        let rows = self
            .db
            .highlights(doc, range.start, range.end)
            .map_err(SourceError::new)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                Span::from(row.start..row.end)
                    .with_id(row.id)
                    .with_attr("color", row.color)
            })
            .collect())
    }

    // Optional. Answers a query filter directly, so the query doesn't have to
    // call get_spans over everything it searches.
    fn find_spans(&self, doc: &str, filter: &SpanFilter) -> Option<Result<Vec<Span>, SourceError>> {
        None
    }
}
