//! Documents — text that only grows, with attributes and byte ranges.
//!
//! API sketch only; this does not compile yet.

use klause::{Document, Span};

fn main() -> Result<(), klause::Error> {
    // Built with attributes that describe the document itself.
    let mut doc = Document::new("KAI 181")
        .with_attr("language", "Moabite")
        .with_attr("site", "Dhiban")
        .with_attr("date", -840); // years as integers, negative for BCE

    // push appends and returns the byte range of what it added, so existing
    // positions never move.
    let a = doc.push("ʾnk")?; // 0..4: positions are bytes, and ʾ takes two
    doc.push(" . ")?;
    let b = doc.push("mšʿ")?; // 7..12

    assert_eq!(a, 0..4);
    assert_eq!(b, 7..12);
    assert_eq!(doc.len(), 12);
    assert_eq!(doc.text(), "ʾnk . mšʿ");
    assert_eq!(doc.slice(b), "mšʿ");
    doc.attr("language"); // Some(&Value::Str("Moabite"))

    // Text and a span in one step; the closure receives the new span.
    doc.push_span("words", "bn", |s| s.with_id("1.3").with_attr("restored", false))?;
    doc.push_span("words", "kmšyt", |s| s)?; // no ID or attributes

    // Spans covering several pushes are added afterwards from recorded positions.
    let start = doc.len();
    // … push the words of the line …
    doc.add_span("lines", Span::from(start..doc.len()).with_id("1"))?;

    // Each push is normalised to NFC, and the returned range points into the
    // normalised text. A push may not start with a combining mark.
    doc.push("\u{05B7}")?; // Err(SplitsCharacter)

    Ok(())
}
