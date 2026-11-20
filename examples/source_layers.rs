//! Source layers — spans read live from your own storage, which Klause never writes to.
//!
//! API sketch only; this does not compile yet.

use std::ops::Range;

use klause::{Document, Layer, Span, SpanFilter, SpanSource, SourceError};

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

fn main() -> Result<(), klause::Error> {
    let mut doc = Document::new("GEN");
    doc.add_layer(Layer::from_source(Highlights { db: Db::open() }))?;

    // A change saved in your database shows up in the next passage or query.
    let p = doc.get_passage(..)?;
    p.layer("highlights")?.error(); // Some(SourceError) if the database failed

    // A source failure doesn't lose the text: the passage still loads and that
    // layer's entry reports the error.
    let p = doc.get_passage(..)?;
    p.text(); // present even when a source failed

    // A query can't give correct results without the source, so unlike a
    // passage, it fails.
    doc.find().in_layer("highlights").get_selection()?; // Err(Error::Source { .. })

    Ok(())
}
