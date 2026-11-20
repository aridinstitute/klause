//! Errors — one enum, raised by the final verb rather than by fluent steps.
//!
//! API sketch only; this does not compile yet.

use std::ops::Range;

use klause::{FormatError, SourceError, SpanId, SpanRef};

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

fn main() -> Result<(), Error> {
    let doc = klause::Document::new("GEN");

    // Fluent steps never fail: a bad layer or range surfaces from the final verb.
    let hits = doc
        .find()
        .covered_by(klause::spans("morph").attr("pos", "noun"))
        .get_selection()?; // Err(UnknownLayer("morph")) when the layer is missing

    hits.count();

    Ok(())
}
