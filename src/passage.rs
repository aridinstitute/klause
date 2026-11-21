use alloc::vec::Vec;
use core::ops::{Deref, Range};

use crate::document::{Span, SpanRef, pending_iter};
use crate::source::SourceError;

/// One contiguous range of document text and the spans in included layers that touch it.
#[derive(Clone, Debug)]
pub struct Passage {
    _private: (),
}

impl Passage {
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
        todo!()
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
        todo!()
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
        pending_iter()
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
    pub fn spans_in(&self, _layer: &str) -> impl Iterator<Item = &PassageSpan> {
        pending_iter()
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
    pub fn error(&self, _layer: &str) -> Option<&SourceError> {
        todo!()
    }
}

/// A span annotation viewed within a [`Passage`], with ranges clipped and measured relative to the passage.
#[derive(Clone, Debug)]
pub struct PassageSpan {
    _private: (),
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
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
    }

    /// Returns a reference to the original span in the document.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, select_span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # let passage = doc.get_passage(15..21)?;
    /// # let annotation = passage.spans_in("verses").next().unwrap();
    /// assert_eq!(annotation.to_ref(), select_span("verses", "GEN 3:15"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn to_ref(&self) -> SpanRef {
        todo!()
    }
}

impl Deref for PassageSpan {
    type Target = Span;

    /// Returns the underlying span annotation.
    fn deref(&self) -> &Span {
        todo!()
    }
}
