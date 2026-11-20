//! Saving and loading — documents, single layers, and corpora.
//!
//! API sketch only; this does not compile yet.

use klause::{Corpus, Document, Layer};

fn main() -> Result<(), klause::Error> {
    let doc = Document::new("KAI 181");

    // A document: text, attributes, stored layers, and indexes.
    doc.save("kai-181.klause")?;
    let doc = Document::open("kai-181.klause")?; // indexes are loaded, not rebuilt

    // One layer on its own, to share or version separately.
    doc.save_layer("morph", "kai-181.morph.klause")?;
    doc.add_layer(Layer::open("kai-181.morph.klause")?)?;

    // A corpus: every document, plus the corpus's layers.
    let lib = Corpus::new();
    lib.save("library/")?;
    let lib = Corpus::open("library/")?;

    // Source layers are never saved; they belong to you. The file format is
    // versioned.

    Ok(())
}
