//! Layers — named sets of spans, stored in the document or read live from your storage.
//!
//! API sketch only; this does not compile yet.

use klause::{Document, Layer, Span};

fn main() -> Result<(), klause::Error> {
    let mut doc = Document::new("KAI 181");

    // A stored layer, with metadata about who made it.
    doc.add_layer(
        Layer::new("morph")
            .with_name("Morphology")
            .with_attr("creator", "A. Scholar")
            .with_attr("license", "CC-BY-4.0"),
    )?;

    // A source layer reads from your own storage; see source_layers.rs.
    doc.add_layer(Layer::from_source(highlights))?;

    doc.layer("morph")?.name(); // "Morphology"
    doc.layer("morph")?.attr("license"); // Some(&Value::Str("CC-BY-4.0"))
    doc.layers(); // in the order they were added

    // Layer IDs can't contain dots, because filters write "layer.key".
    doc.add_layer(Layer::new("bad.id"))?; // Err(...)

    // Adding a span to a missing stored layer creates it implicitly. Is that
    // the behaviour we want, or should it require add_layer first?
    doc.add_span("notes", Span::from(0..9))?;

    Ok(())
}
