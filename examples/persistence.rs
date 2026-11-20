//! Persistence and errors — anchors, saving and loading, and what can fail.
//!
//! API sketch only; this does not compile yet.

use std::ops::Range;

use klause::{Corpus, Document, FormatError, Layer, SourceError, Span, SpanId, SpanRef};

fn main() -> Result<(), klause::Error> {
    // -- Anchors -----------------------------------------------------------
    // Stored positions stay correct for as long as you keep the document. A new
    // edition is a new document, so save an anchor alongside any span you store
    // outside the document.
    let doc = Document::new("GEN");
    let target = Span::from(0..47);
    let anchor = doc.anchor(&target)?; // Anchor { checksum, quote, prefix, suffix }
    db.save(&target, &anchor)?;

    // quote is the covered text; prefix and suffix are a few words on either
    // side. Together they're enough to find the text again by searching.
    anchor.quote();
    anchor.prefix();
    anchor.suffix();

    let new_edition = Document::new("GEN 2nd ed.");
    new_edition.verify(&target, &anchor)?; // Ok(()) or Err(Stale)

    // -- Saving and loading ------------------------------------------------
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

    // Source layers are never saved; they belong to you. The format is versioned.

    Ok(())
}

// -- Errors ----------------------------------------------------------------
// Fluent steps never fail; errors surface from the final verb or the call that
// does the work.
enum Error {
    UnknownDocument(String),
    UnknownLayer(String),
    UnknownSpan(SpanRef),
    DuplicateId { layer: String, id: SpanId },
    OutOfRange(Range<u32>),
    NotCharBoundary(u32),
    SplitsCharacter,                              // a push that starts with a combining mark
    DocumentRequired,                             // a corpus-layer span that doesn't name its document
    ReadOnlyLayer(String),                        // editing a source layer
    NotOne { found: usize },                      // get_passage on zero or several passages
    Source { layer: String, error: SourceError }, // a query needed a source that failed
    Format(FormatError),                          // reading or writing a saved file
}
