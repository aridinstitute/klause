use alloc::{string::String, vec::Vec};
use core::ops::{Range, RangeFrom, RangeFull};

use crate::document::{Document, Error, SpanId, SpanRef, Value};

/// A set of text ranges in a document, represented as sorted, non-overlapping byte ranges.
/// Unlike a [`crate::Span`], a selection describes text positions and has no annotation attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    _private: (),
}

impl Selection {
    /// Returns the normalized text ranges in this selection.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// let word = doc.get_selection(15..21)?;
    /// assert_eq!(word.ranges(), &[15..21]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn ranges(&self) -> &[Range<u32>] {
        todo!()
    }

    /// Returns a selection containing ranges from either selection.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// let put = doc.get_selection(11..14)?;
    /// let enmity = doc.get_selection(15..21)?;
    /// assert_eq!(put.union(&enmity).ranges(), &[11..14, 15..21]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn union(&self, _other: &Selection) -> Selection {
        todo!()
    }

    /// Returns the text covered by both selections.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// let phrase = doc.get_selection(11..21)?;
    /// let word = doc.get_selection(15..21)?;
    /// assert_eq!(phrase.intersect(&word).ranges(), &[15..21]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn intersect(&self, _other: &Selection) -> Selection {
        todo!()
    }

    /// Returns the portions of this selection not covered by `other`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// let phrase = doc.get_selection(11..21)?;
    /// let word = doc.get_selection(15..21)?;
    /// assert_eq!(phrase.subtract(&word).ranges(), &[11..15]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn subtract(&self, _other: &Selection) -> Selection {
        todo!()
    }

    /// Returns whether the byte position is selected, either by a range or as a point.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// let word = doc.get_selection(15..21)?;
    /// assert!(word.contains(15)); // The start of “enmity”.
    /// assert!(!word.contains(21)); // Just after “enmity”.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn contains(&self, _position: u32) -> bool {
        todo!()
    }

    /// Returns whether this selection contains no ranges.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// let word = doc.get_selection(15..21)?;
    /// assert!(word.subtract(&word).is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn is_empty(&self) -> bool {
        todo!()
    }
}

/// Text described against a document, resolved when a query step or passage runs.
pub trait IntoSelection {
    /// Resolves this description into text ranges for the given document.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, IntoSelection, Span, select_span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let reference = select_span("verses", "GEN 3:15");
    /// let selection = reference.into_selection(&doc)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn into_selection(self, doc: &Document) -> Result<Selection, Error>;
}

macro_rules! into_selection {
    ($($source:ty),*) => {$(
        impl IntoSelection for $source {
            fn into_selection(self, _doc: &Document) -> Result<Selection, Error> {
                todo!()
            }
        }
    )*};
}

into_selection!(
    Range<u32>,
    RangeFull,
    Vec<Range<u32>>,
    Selection,
    &Selection,
    SpanRef,
    SpanFilter
);

impl<const N: usize> IntoSelection for [Range<u32>; N] {
    fn into_selection(self, _doc: &Document) -> Result<Selection, Error> {
        todo!()
    }
}

/// Criteria for matching spans, such as spans in `morph` whose `pos` is `verb`.
#[derive(Clone, Debug)]
pub struct SpanFilter {
    _private: (),
}

impl SpanFilter {
    /// Requires spans to have an attribute matching the supplied matcher.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, select_spans};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let nouns = select_spans("words").with_attr("pos", "noun");
    /// let verbs = select_spans("words").with_attr("pos", "verb");
    /// assert_eq!(doc.get_selection(nouns)?.ranges(), &[15..21]); // “enmity”
    /// assert!(doc.get_selection(verbs)?.is_empty()); // No verb is tagged.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_attr(self, _key: impl Into<String>, _matcher: impl ValueMatcher) -> Self {
        todo!()
    }

    /// Excludes spans having the specified attribute value.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, select_spans};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let non_verbs = select_spans("words").without_attr("pos", "verb");
    /// let hits = doc.get_selection(non_verbs)?; // “enmity” is still selected: it is tagged as a noun.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn without_attr(self, _key: impl Into<String>, _value: impl Into<Value>) -> Self {
        todo!()
    }

    /// Requires spans to contain the specified attribute key.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, select_spans};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let tagged_words = select_spans("words").with_key("pos");
    /// let hits = doc.get_selection(tagged_words)?; // Only the tagged word “enmity” is selected.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_key(self, _key: impl Into<String>) -> Self {
        todo!()
    }

    /// Excludes spans containing the specified attribute key.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, select_spans};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let untagged_words = select_spans("words").without_key("pos");
    /// let hits = doc.get_selection(untagged_words)?; // The untagged word “put” is selected.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn without_key(self, _key: impl Into<String>) -> Self {
        todo!()
    }

    /// Restricts matching to spans with the specified identifier.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, select_spans};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let enmity = select_spans("words").with_id("enmity");
    /// let hits = doc.get_selection(enmity)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_id(self, _id: impl Into<SpanId>) -> Self {
        todo!()
    }

    /// Restricts matching to spans whose target matches another filter.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, select_spans};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// # doc.add_layer("notes")?;
    /// # doc.add_span("notes", Span::on_ref(&verse).with_attr("topic", "enmity"))?;
    /// let verse = select_spans("verses").with_id("GEN 3:15");
    /// let notes = select_spans("notes").targeting(verse);
    /// let hits = doc.get_selection(notes)?; // The note on Genesis 3:15 is selected.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn targeting(self, _target: SpanFilter) -> Self {
        todo!()
    }
}

impl From<&str> for SpanFilter {
    /// Creates a filter for spans in the named layer.
    fn from(_layer: &str) -> Self {
        todo!()
    }
}

/// A condition on an attribute value, such as equality, a set of values, or a numeric range.
pub trait ValueMatcher {
    /// Returns whether this matcher accepts the given attribute value.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Value, ValueMatcher, any_of};
    /// let topic = any_of(["enmity", "offspring"]);
    /// assert!(topic.matches(&Value::from("enmity")));
    /// assert!(!topic.matches(&Value::from("garden")));
    /// ```
    fn matches(&self, value: &Value) -> bool;
}

macro_rules! value_matcher {
    ($($source:ty),*) => {$(
        impl ValueMatcher for $source {
            fn matches(&self, _value: &Value) -> bool {
                todo!()
            }
        }
    )*};
}

value_matcher!(
    Value,
    &str,
    String,
    i32,
    i64,
    f64,
    bool,
    AnyOf,
    Range<i32>,
    RangeFrom<i32>,
    Range<i64>,
    RangeFrom<i64>,
    Range<f64>,
    RangeFrom<f64>
);

/// A value matcher that accepts any of the values supplied to [`any_of`].
#[derive(Clone, Debug)]
pub struct AnyOf {
    _private: (),
}

/// Creates a matcher that accepts any of the supplied values.
///
/// # Example
///
/// ```no_run
/// # use klause::{any_of, select_spans};
/// let words = select_spans("words").with_attr("pos", any_of(["noun", "verb"]));
/// ```
pub fn any_of<T: Into<Value>>(_values: impl IntoIterator<Item = T>) -> AnyOf {
    todo!()
}

/// Creates a reference to one span by layer and identifier.
///
/// # Example
///
/// ```no_run
/// # use klause::{Document, Span, select_span};
/// # let mut doc = Document::new("GEN 3:15");
/// # let range = doc.push("And I will put enmity")?;
/// # doc.add_layer("verses")?;
/// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
/// let verse = select_span("verses", "GEN 3:15");
/// let passage = doc.get_passage(verse)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn select_span(_layer: impl Into<String>, _id: impl Into<SpanId>) -> SpanRef {
    todo!()
}

/// Creates a filter matching spans in the named layer.
///
/// # Example
///
/// ```no_run
/// # use klause::{Document, Span, select_spans};
/// # let mut doc = Document::new("GEN 3:15");
/// # let range = doc.push("And I will put enmity")?;
/// # doc.add_layer("verses")?;
/// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
/// let verses = select_spans("verses");
/// let hits = doc.get_selection(verses)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn select_spans(_layer: impl Into<String>) -> SpanFilter {
    todo!()
}

/// Creates a filter matching spans in every layer.
///
/// # Example
///
/// ```no_run
/// # use klause::{Document, Span, select_all_spans};
/// # let mut doc = Document::new("GEN 3:15");
/// # let range = doc.push("And I will put enmity")?;
/// # doc.add_layer("verses")?;
/// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
/// let hits = doc.get_selection(select_all_spans())?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn select_all_spans() -> SpanFilter {
    todo!()
}
