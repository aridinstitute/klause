use alloc::{string::String, vec::Vec};
use core::marker::PhantomData;

use crate::document::{Document, Error, Span, SpanId, Value};
use crate::passage::Passage;
use crate::selection::{IntoSelection, Selection, SpanFilter};

/// A document query that starts with the whole text and applies steps to narrow or widen the match ranges.
/// The steps run when a result is requested, for example with [`Query::get_selection`].
pub struct Query<'a> {
    _doc: PhantomData<&'a Document>,
}

impl<'a> Query<'a> {
    /// Keeps only the portions of the current hits within the given selection.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// assert_eq!(doc.find().with_text("enmity").within(15..21).count()?, 1);
    /// assert_eq!(doc.find().with_text("enmity").within(0..14).count()?, 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn within(self, _selection: impl IntoSelection + 'a) -> Self {
        todo!()
    }

    /// Removes the portions of the current hits within the given selection.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// assert_eq!(doc.find().with_text("enmity").outside(0..14).count()?, 1);
    /// assert_eq!(doc.find().with_text("enmity").outside(15..21).count()?, 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn outside(self, _selection: impl IntoSelection + 'a) -> Self {
        todo!()
    }

    /// Keeps only the portions of the current hits covered by spans in the given layer.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let hits = doc.find().in_layer("verses").get_selection()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn in_layer(self, _layer: impl Into<String>) -> Self {
        todo!()
    }

    /// Keeps only the portions of the current hits covered by the specified span.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let hits = doc.find().in_span("verses", "GEN 3:15").get_selection()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn in_span(self, _layer: impl Into<String>, _id: impl Into<SpanId>) -> Self {
        todo!()
    }

    /// Finds literal occurrences of the text wholly within the current hit ranges.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// assert_eq!(doc.find().with_text("enmity").count()?, 1);
    /// assert_eq!(doc.find().with_text("ENMITY").count()?, 0); // Case matters.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_text(self, _text: &str) -> Self {
        todo!()
    }

    /// Finds occurrences within the current hit ranges, ignoring case and nonspacing combining marks.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// assert_eq!(doc.find().with_text_folded("ENMITY").count()?, 1);
    /// assert_eq!(doc.find().with_text_folded("enemy").count()?, 0); // Not a synonym search.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_text_folded(self, _text: &str) -> Self {
        todo!()
    }

    /// Keeps hits followed by a matching span range within the given distance.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Distance, Document, Span, select_spans};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let enmity = select_spans("words").with_id("enmity");
    /// let hits = doc.find().with_text("put")
    ///     .followed_by(enmity.clone(), Distance::adjacent())
    ///     .count()?;
    /// assert_eq!(hits, 1); // “put” is followed by “enmity”.
    ///
    /// let hits = doc.find().with_text("will")
    ///     .followed_by(enmity, Distance::adjacent())
    ///     .count()?;
    /// assert_eq!(hits, 0); // “put” comes between “will” and “enmity”.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn followed_by(self, _filter: impl Into<SpanFilter>, _distance: Distance) -> Self {
        todo!()
    }

    /// Keeps hits preceded by a matching span range within the given distance.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Distance, Document, Span, select_spans};
    /// for (text, expected) in [
    ///     ("I will put enmity between you and the woman", 1),
    ///     ("There is enmity between you and the woman", 0),
    /// ] {
    ///     let mut doc = Document::new("GEN 3:15").with_layer("words");
    ///     doc.push(text)?;
    ///     if let Some(start) = text.find("put") {
    ///         let start = start as u32;
    ///         doc.add_span("words", Span::from(start..start + 3).with_id("put"))?;
    ///     }
    ///     let hits = doc.find().with_text("enmity")
    ///         .preceded_by(select_spans("words").with_id("put"), Distance::adjacent())
    ///         .count()?;
    ///     assert_eq!(hits, expected); // Only the first text matches.
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn preceded_by(self, _filter: impl Into<SpanFilter>, _distance: Distance) -> Self {
        todo!()
    }

    /// Replaces the hits with the union of ranges from matching spans that touch a hit.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let passage = doc.find().with_text("enmity").expand_to("verses").get_passage()?; // The passage now covers the whole verse annotation.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn expand_to(self, _filter: impl Into<SpanFilter>) -> Self {
        todo!()
    }

    /// Limits the annotation layers included in passages without changing which text matches.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let passage = doc.find().with_text("enmity").with_layers(["verses"]).get_passage()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_layers<I, S>(self, _layers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        todo!()
    }

    /// Runs the query and returns its matching text ranges as a selection.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// let hits = doc.find().with_text("enmity").get_selection()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_selection(self) -> Result<Selection, Error> {
        todo!()
    }

    /// Runs the query and returns one passage per matching range.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// let passages = doc.find().with_text("enmity").get_passages()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_passages(self) -> Result<Vec<Passage>, Error> {
        todo!()
    }

    /// Runs the query and returns a passage if there is exactly one matching range.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// let passage = doc.find().with_text("enmity").get_passage()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_passage(self) -> Result<Passage, Error> {
        todo!()
    }

    /// Returns spans that match the filter and touch at least one hit.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let verses = doc.find().with_text("enmity").get_spans("verses")?; // The Genesis 3:15 annotation touches the hit on “enmity”.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_spans(self, _filter: impl Into<SpanFilter>) -> Result<Vec<Span>, Error> {
        todo!()
    }

    /// Returns the number of matching text ranges.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// assert_eq!(doc.find().with_text("enmity").count()?, 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn count(self) -> Result<usize, Error> {
        todo!()
    }

    /// Counts spans that match the filter and touch at least one hit.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// assert_eq!(doc.find().with_text("enmity").count_spans("verses")?, 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn count_spans(self, _filter: impl Into<SpanFilter>) -> Result<usize, Error> {
        todo!()
    }

    /// Counts how many spans touching a hit have each value for the given layer and key.
    /// Returns the most frequent values first.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let counts = doc.find().with_text("enmity").count_values("words", "pos")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn count_values(self, _layer: &str, _key: &str) -> Result<Vec<(Value, usize)>, Error> {
        todo!()
    }
}

/// How far apart the two parts of `followed_by` and `preceded_by` may be.
#[derive(Clone, Debug)]
pub struct Distance {
    _private: (),
}

impl Distance {
    /// Only whitespace and punctuation in between.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Distance, Document, Span, select_spans};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let hits = doc.find().with_text("put")
    ///     .followed_by(select_spans("words").with_id("enmity"), Distance::adjacent())
    ///     .get_selection()?; // Only a space separates “put” and “enmity”.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn adjacent() -> Self {
        todo!()
    }

    /// At most `n` spans of `layer` lie wholly in between.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Distance, Document, Span, select_spans};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let hits = doc.find().with_text("will")
    ///     .followed_by(select_spans("words").with_id("enmity"), Distance::at_most(1, "words"))
    ///     .get_selection()?; // “will” matches with one intervening word: “put”.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn at_most(_n: usize, _layer: impl Into<String>) -> Self {
        todo!()
    }

    /// At most `n` bytes in between.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Distance, Document, Span, select_spans};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let hits = doc.find().with_text("put")
    ///     .followed_by(select_spans("words").with_id("enmity"), Distance::bytes(1))
    ///     .get_selection()?; // “put” matches across the one-byte space.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn bytes(_n: u32) -> Self {
        todo!()
    }
}
