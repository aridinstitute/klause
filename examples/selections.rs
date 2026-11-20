//! Selections — sorted sets of text ranges, with set operations.
//!
//! API sketch only; this does not compile yet.

use klause::Document;

fn main() -> Result<(), klause::Error> {
    let doc = Document::new("GEN");

    let a = doc.get_selection(0..47)?;
    let b = doc.get_selection([42..47, 61..70])?;

    a.union(&b); // each returns a new Selection
    a.intersect(&b);
    a.subtract(&b);

    b.ranges(); // [("GEN", 42..47), ("GEN", 61..70)]
    b.contains("GEN", 44); // true
    b.is_empty();

    // Plain data with no borrow of the document: store it, compare it, and pass
    // it back in. Anything IntoSelection works: ranges, `..`, arrays of ranges,
    // SpanRef, &Span, link ends, and Selection itself.
    doc.get_passage(&b)?;

    Ok(())
}
