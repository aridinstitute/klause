//! Corpora — several documents, queried together, with links between them.
//!
//! API sketch only; this does not compile yet.

use klause::{Corpus, Layer, Span, SpanRef, docs, spans};

fn main() -> Result<(), klause::Error> {
    let mut lib = Corpus::new();
    lib.add_document(gen)?;
    lib.add_document(isa)?;
    lib.add_document(rom)?;

    // Layers shared by every document.
    lib.add_layer(Layer::new("echoes"))?; // a corpus layer
    lib.add_layer(Layer::from_source(highlights))?; // asked about every document

    lib.document("ROM")?.len();
    lib.document_mut("ROM")?.add_span("notes", Span::from(0..9))?; // documents keep their own layers

    // Spans in a corpus layer must say which document they're in.
    lib.add_span(
        "echoes",
        Span::link(
            SpanRef::new("verses", "10:15").in_doc("ROM"),
            SpanRef::new("verses", "52:7").in_doc("ISA"),
        )
        .with_attr("kind", "quotation"),
    )?;

    lib.add_span("echoes", Span::link(("ROM", 2_210..2_301), ("ISA", 41_880..41_940)))?;

    // Passages and queries work as on a document. They include each document's
    // own layers and the corpus's layers.
    lib.get_passage("KAI 181")?; // a document ID: the whole document
    lib.get_passage(("GEN", 0..47))?;
    lib.get_passage(SpanRef::new("verses", "3:15").in_doc("GEN"))?;
    lib.get_selection(["ROM", "1CO", "2CO", "GAL"])?; // several whole documents

    lib.find()
        .within(docs().attr("language", "Moabite").attr("date", ..-800)) // a document filter
        .covered_by(spans("morph").attr("pos", "verb"))
        .count_values("morph.lemma")?;

    Ok(())
}
