use alloc::{string::String, sync::Arc, vec::Vec};
use core::ops::{Deref, Range};

use crate::document::{Span, SpanRef};
use crate::source::SourceError;

/// One contiguous range of document text and the spans in included layers that touch it.
#[derive(Clone, Debug)]
pub struct Passage {
    text: Arc<String>,
    range: Range<u32>,
    spans: Vec<PassageSpan>,
    errors: Vec<(String, Arc<SourceError>)>,
}

impl Passage {
    pub(crate) fn new(
        text: Arc<String>,
        range: Range<u32>,
        spans: Vec<PassageSpan>,
        errors: Vec<(String, Arc<SourceError>)>,
    ) -> Self {
        Self {
            text,
            range,
            spans,
            errors,
        }
    }

    /// Returns the passage's byte range in the document.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// assert_eq!(passage.range(), 15..21);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn range(&self) -> Range<u32> {
        self.range.clone()
    }

    /// Returns the text of this passage.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// assert_eq!(passage.text(), "enmity");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn text(&self) -> &str {
        &self.text[self.range.start as usize..self.range.end as usize]
    }

    /// Iterates over all spans that touch this passage in the included layers.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// for annotation in passage.spans() {
    ///     println!("{}", annotation.layer());
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn spans(&self) -> impl Iterator<Item = &PassageSpan> {
        self.spans.iter()
    }

    /// Iterates over the passage spans from the specified layer.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// assert_eq!(passage.spans_in("verses").count(), 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn spans_in(&self, layer: &str) -> impl Iterator<Item = &PassageSpan> {
        self.spans.iter().filter(move |span| span.layer() == layer)
    }

    /// Returns the error from loading the given source layer, if it failed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// assert!(passage.error("verses").is_none());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn error(&self, layer: &str) -> Option<&SourceError> {
        self.errors
            .iter()
            .find(|(id, _)| id == layer)
            .map(|(_, error)| error.as_ref())
    }
}

/// A span annotation viewed within a [`Passage`], with ranges clipped and measured relative to the passage.
#[derive(Clone, Debug)]
pub struct PassageSpan {
    text: Arc<String>,
    passage_range: Range<u32>,
    layer: String,
    span: Span,
    local: Vec<Range<u32>>,
    starts_inside: bool,
    ends_inside: bool,
}

impl PassageSpan {
    pub(crate) fn new(
        text: Arc<String>,
        passage_range: Range<u32>,
        layer: String,
        span: Span,
        ranges: &[Range<u32>],
    ) -> Option<Self> {
        let mut local = Vec::new();
        for range in ranges {
            if range.start == range.end {
                if passage_range.start <= range.start && range.start <= passage_range.end {
                    let position = range.start - passage_range.start;
                    local.push(position..position);
                }
            } else {
                let start = range.start.max(passage_range.start);
                let end = range.end.min(passage_range.end);
                if start < end {
                    local.push(start - passage_range.start..end - passage_range.start);
                }
            }
        }
        if local.is_empty() {
            return None;
        }
        Some(Self {
            text,
            starts_inside: ranges
                .first()
                .is_some_and(|range| range.start >= passage_range.start),
            ends_inside: ranges
                .last()
                .is_some_and(|range| range.end <= passage_range.end),
            passage_range,
            layer,
            span,
            local,
        })
    }
}

impl PassageSpan {
    /// Returns the identifier of the layer that contains this span.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// # let annotation = passage.spans_in("verses").next().unwrap();
    /// assert_eq!(annotation.layer(), "verses");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn layer(&self) -> &str {
        &self.layer
    }

    /// Returns the span's byte ranges clipped to the passage and relative to its start.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// # let annotation = passage.spans_in("verses").next().unwrap();
    /// assert_eq!(annotation.local(), &[0..6]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn local(&self) -> &[Range<u32>] {
        &self.local
    }

    /// Returns one text fragment for each local range of this span.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// # let annotation = passage.spans_in("verses").next().unwrap();
    /// assert_eq!(annotation.text(), ["enmity"]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn text(&self) -> Vec<&str> {
        self.local
            .iter()
            .map(|range| {
                let start = self.passage_range.start + range.start;
                let end = self.passage_range.start + range.end;
                &self.text[start as usize..end as usize]
            })
            .collect()
    }

    /// Returns whether the first range of the span starts inside the passage.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// # let annotation = passage.spans_in("verses").next().unwrap();
    /// assert!(!annotation.starts_inside());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn starts_inside(&self) -> bool {
        self.starts_inside
    }

    /// Returns whether the last range of the span ends inside the passage.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// # let annotation = passage.spans_in("verses").next().unwrap();
    /// assert!(annotation.ends_inside()); // The span and passage end together.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn ends_inside(&self) -> bool {
        self.ends_inside
    }

    /// Returns a reference to the original span in the document.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, span_id};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// # let annotation = passage.spans_in("verses").next().unwrap();
    /// assert_eq!(annotation.to_ref(), span_id("verses", "GEN 3:15"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn to_ref(&self) -> SpanRef {
        SpanRef::new(
            self.layer.clone(),
            self.span
                .id()
                .expect("stored and source spans in passages have IDs")
                .clone(),
        )
    }
}

impl Deref for PassageSpan {
    type Target = Span;

    /// Returns the underlying span annotation.
    fn deref(&self) -> &Span {
        &self.span
    }
}
