//! Anchors — keep a stored span findable when a text gets a new edition.
//!
//! API sketch only; this does not compile yet.

use klause::{Document, Span};

fn main() -> Result<(), klause::Error> {
    let doc = Document::new("GEN");
    // … text pushed here …

    // A document's existing text never changes, so stored positions stay correct
    // for as long as you keep the document. Save an anchor alongside any span
    // you store outside it.
    let target = Span::from(0..47);
    let anchor = doc.anchor(&target)?; // Anchor { checksum, quote, prefix, suffix }
    db.save(&target, &anchor)?;

    // quote is the covered text; prefix and suffix are a few words on either
    // side. Together they're enough to find the text again by searching.
    anchor.quote(); // "And I will put enmity between you and the woman"
    anchor.prefix();
    anchor.suffix();

    // A new edition of a text is a new document, and spans saved against the old
    // one may no longer fit.
    let new_edition = Document::new("GEN 2nd ed.");
    new_edition.verify(&target, &anchor)?; // Ok(()) or Err(Stale)

    Ok(())
}
