use std::ops::Range;

use klause::{Document, Error, IntoDocument, Layer, Span, SpanId, SpanRef, Target, Value};

#[test]
fn pushes_return_byte_ranges_and_keep_earlier_positions() {
    let mut doc = Document::new("GEN");
    assert!(doc.is_empty());

    let first = doc.push("In").unwrap();
    assert_eq!(first, 0..2);
    assert_eq!(doc.push(" ").unwrap(), 2..3);
    assert_eq!(doc.push("א").unwrap(), 3..5);
    assert_eq!(doc.push("").unwrap(), 5..5);
    assert_eq!(doc.text(), "In א");
    assert_eq!(doc.len(), 5);
    assert!(!doc.is_empty());
    assert_eq!(doc.slice(first), "In");
    assert_eq!(doc.slice(3..5), "א");
}

#[test]
fn push_returns_positions_in_nfc_text() {
    let mut doc = Document::new("nfc");
    doc.push(" ").unwrap();

    let range = doc.push("e\u{0301}").unwrap();
    assert_eq!(range, 1..3);
    assert_eq!(doc.slice(range), "é");
    assert_eq!(doc.text(), " é");
}

#[test]
fn push_accepts_combining_marks_that_leave_earlier_text_unchanged() {
    let mut doc = Document::new("nfc");

    // Nothing composes with a space.
    doc.push(" ").unwrap();
    assert_eq!(doc.push("\u{0301}").unwrap(), 1..3);

    // Alef with patah has no composed form in NFC.
    let alef = doc.push("א").unwrap();
    assert_eq!(doc.push("\u{05B7}").unwrap(), 5..7);

    // Etnahta (combining class 220) already sorts after patah (17).
    assert_eq!(doc.push("\u{0591}").unwrap(), 7..9);
    assert_eq!(doc.text(), " \u{0301}א\u{05B7}\u{0591}");
    assert_eq!(doc.slice(alef), "א");
}

#[test]
fn push_rejects_normalization_that_would_change_existing_text() {
    let mut doc = Document::new("nfc");
    let range = doc.push("e").unwrap();

    // Would compose with the "e" into "é".
    assert!(matches!(doc.push("\u{0301}"), Err(Error::SplitsCharacter)));
    assert_eq!(doc.text(), "e");
    assert_eq!(doc.slice(range), "e");
    assert_eq!(doc.push("!").unwrap(), 1..2);

    // Patah (combining class 17) would be reordered before the etnahta (220) already pushed.
    doc.push("א\u{0591}").unwrap();
    assert!(matches!(doc.push("\u{05B7}"), Err(Error::SplitsCharacter)));
    assert_eq!(doc.text(), "e!א\u{0591}");
}

#[test]
fn layers_are_read_back_in_insertion_order() {
    let mut doc = Document::new("GEN")
        .with_layer("words")
        .with_layer(Layer::new("morph").with_name("Morphology"));
    doc.add_layer("verses").unwrap();

    assert_eq!(doc.layer("morph").unwrap().name(), "Morphology");
    assert!(!doc.layer("words").unwrap().is_source());
    assert_eq!(doc.layer("words").unwrap().spans().count(), 0);
    assert!(doc.layer("missing").is_none());
    assert_eq!(
        doc.layers().map(Layer::id).collect::<Vec<_>>(),
        ["words", "morph", "verses"]
    );
}

#[test]
fn adding_a_duplicate_layer_is_an_error() {
    let mut doc = Document::new("GEN").with_layer("words");

    assert!(matches!(
        doc.add_layer("words"),
        Err(Error::DuplicateLayer(id)) if id == "words"
    ));
    assert_eq!(doc.layers().count(), 1);
}

#[test]
#[should_panic]
fn building_with_a_duplicate_layer_panics() {
    let _ = Document::new("GEN").with_layer("words").with_layer("words");
}

#[test]
fn push_span_annotates_normalized_text_and_assigns_missing_ids() {
    let mut doc = Document::new("GEN").with_layer("words");
    let first = doc
        .push_span("words", "Cafe\u{0301}", |span| {
            span.with_id(10_u64).with_attr("pos", "noun")
        })
        .unwrap();
    doc.push(" ").unwrap();
    let second = doc.push_span("words", "noir", |span| span).unwrap();

    assert_eq!(doc.text(), "Café noir");
    assert_eq!(first, 0..5);
    assert_eq!(second, 6..10);
    let first_ref = SpanRef::new("words", 10_u64);
    assert_eq!(doc.ranges(&first_ref), Some(std::slice::from_ref(&first)));
    assert_eq!(
        doc.span(&first_ref).unwrap().attr("pos"),
        Some(&Value::from("noun"))
    );
    assert_eq!(
        doc.ranges(&SpanRef::new("words", 11_u64)),
        Some(std::slice::from_ref(&second))
    );
    assert_eq!(doc.layer("words").unwrap().spans().count(), 2);
}

#[test]
fn adding_spans_rejects_unknown_layers_bad_ranges_and_duplicate_ids() {
    let mut doc = Document::new("GEN").with_layer("words");

    assert!(matches!(
        doc.push_span("verses", "In", |span| span),
        Err(Error::UnknownLayer(id)) if id == "verses"
    ));
    assert_eq!(doc.text(), "");

    doc.push("é").unwrap();
    assert!(matches!(
        doc.add_span("words", Span::from(0..3)),
        Err(Error::OutOfRange(range)) if range == (0..3)
    ));
    assert!(matches!(
        doc.add_span("words", Span::from(0..1)),
        Err(Error::NotCharBoundary(1))
    ));
    doc.add_span("words", Span::from(0..2).with_id(1_u64))
        .unwrap();
    assert!(matches!(
        doc.add_span("words", Span::from(0..2).with_id(1_u64)),
        Err(Error::DuplicateId(reference))
            if reference.layer() == "words" && reference.id() == &SpanId::Int(1)
    ));
    assert_eq!(doc.layer("words").unwrap().spans().count(), 1);
}

/// A document as an application would store it, outside Klause.
#[derive(Clone, Debug, PartialEq)]
struct DocumentRow {
    id: String,
    attrs: Vec<(String, Value)>,
    text: String,
    layers: Vec<LayerRow>,
}

#[derive(Clone, Debug, PartialEq)]
struct LayerRow {
    id: String,
    name: String,
    attrs: Vec<(String, Value)>,
    spans: Vec<SpanRow>,
}

#[derive(Clone, Debug, PartialEq)]
struct SpanRow {
    id: SpanId,
    ranges: Vec<Range<u32>>,
    attrs: Vec<(String, Value)>,
}

#[derive(Debug)]
enum StorageError {
    Klause(Error),
}

impl From<Error> for StorageError {
    fn from(error: Error) -> Self {
        StorageError::Klause(error)
    }
}

fn attr_rows<'a>(attrs: impl Iterator<Item = (&'a str, &'a Value)>) -> Vec<(String, Value)> {
    attrs
        .map(|(key, value)| (key.to_owned(), value.clone()))
        .collect()
}

fn write_out(doc: &Document) -> DocumentRow {
    DocumentRow {
        id: doc.id().to_owned(),
        attrs: attr_rows(doc.attrs()),
        text: doc.text().to_owned(),
        layers: doc
            .layers()
            .map(|layer| LayerRow {
                id: layer.id().to_owned(),
                name: layer.name().to_owned(),
                attrs: attr_rows(layer.attrs()),
                spans: layer
                    .spans()
                    .map(|span| SpanRow {
                        id: span.id().unwrap().clone(),
                        ranges: match span.target() {
                            Target::Text(ranges) => ranges.clone(),
                            Target::Span(_) => unreachable!("these fixtures target only text"),
                        },
                        attrs: attr_rows(span.attrs()),
                    })
                    .collect(),
            })
            .collect(),
    }
}

impl IntoDocument for DocumentRow {
    type Error = StorageError;

    fn into_document(self) -> Result<Document, StorageError> {
        let mut doc = self
            .attrs
            .into_iter()
            .fold(Document::new(self.id), |doc, (key, value)| {
                doc.with_attr(key, value)
            });
        doc.push(&self.text)?;
        for row in self.layers {
            let layer = row.attrs.into_iter().fold(
                Layer::new(&row.id).with_name(row.name),
                |layer, (key, value)| layer.with_attr(key, value),
            );
            doc.add_layer(layer)?;
            for span in row.spans {
                let span = span.attrs.into_iter().fold(
                    Span::from(span.ranges).with_id(span.id),
                    |span, (key, value)| span.with_attr(key, value),
                );
                doc.add_span(&row.id, span)?;
            }
        }
        Ok(doc)
    }
}

fn load<T: IntoDocument>(input: T) -> Result<Document, T::Error> {
    input.into_document()
}

#[test]
fn a_document_written_out_through_its_accessors_loads_back_unchanged() {
    let mut doc = Document::new("GEN")
        .with_attr("language", "en")
        .with_attr("title", "Genesis")
        .with_attr("language", "he")
        .with_layer(
            Layer::new("words")
                .with_name("Words")
                .with_attr("source", "BSB"),
        )
        .with_layer("notes");
    doc.push("In the beginning").unwrap();
    // Added out of order: stored spans are read back by start.
    doc.add_span("words", Span::from(7..16).with_id(3_u64))
        .unwrap();
    doc.add_span("words", Span::from(3..6).with_id(2_u64))
        .unwrap();
    doc.add_span(
        "words",
        Span::from(0..2)
            .with_id(1_u64)
            .with_attr("strongs", "H7225")
            .with_attr("position", 1_i64)
            .with_attr("strongs", "H7218"),
    )
    .unwrap();
    doc.add_span("notes", Span::from(vec![0..2, 7..16]).with_id("pair"))
        .unwrap();
    doc.add_span("notes", Span::from(16..16).with_id("marker"))
        .unwrap();
    let doc = load(doc).unwrap();

    let rows = write_out(&doc);
    assert_eq!(
        rows.attrs
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        ["language", "language", "title"]
    );
    assert_eq!(
        rows.layers[0]
            .spans
            .iter()
            .map(|span| &span.id)
            .collect::<Vec<_>>(),
        [&SpanId::Int(1), &SpanId::Int(2), &SpanId::Int(3)]
    );
    assert_eq!(rows.layers[1].spans[0].ranges, [0..2, 7..16]);

    let copy = load(rows.clone()).unwrap();
    assert_eq!(write_out(&copy), rows);
}

#[test]
fn into_document_errors_convert_into_the_callers_error_type() {
    let row = DocumentRow {
        id: "GEN".to_owned(),
        attrs: Vec::new(),
        text: "In".to_owned(),
        layers: vec![LayerRow {
            id: "words".to_owned(),
            name: "Words".to_owned(),
            attrs: Vec::new(),
            spans: vec![SpanRow {
                id: SpanId::Int(1),
                ranges: vec![0..99],
                attrs: Vec::new(),
            }],
        }],
    };

    assert!(matches!(
        load(row),
        Err(StorageError::Klause(Error::OutOfRange(range))) if range == (0..99)
    ));
}
