use alloc::{string::String, sync::Arc, vec, vec::Vec};
use core::{fmt, ops::Range};
use unicode_normalization::UnicodeNormalization;

use crate::layer::Layer;
use crate::passage::Passage;
use crate::query::Query;
use crate::selection::{IntoSelection, Selection};
use crate::source::SourceError;

// Give iterator stubs a concrete return type without choosing storage yet.
pub(crate) fn pending_iter<T>() -> core::iter::Empty<T> {
    todo!()
}

/// A document's UTF-8 text, metadata, and layers of annotations. Text is append-only so existing byte offsets stay valid.
#[derive(Debug)]
pub struct Document {
    id: String,
    text: Arc<String>,
    layers: Vec<Layer>,
    attrs: Attributes,
}

impl Document {
    /// Creates an empty document with the given identifier.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// let doc = Document::new("GEN 3:15");
    /// ```
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: Arc::new(String::new()),
            layers: Vec::new(),
            attrs: Attributes::default(),
        }
    }

    /// Appends values to an attribute on this document.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// let doc = doc.with_attr("reference", "Genesis 3:15");
    /// ```
    pub fn with_attr(mut self, key: impl Into<String>, values: impl IntoValues) -> Self {
        self.attrs.append(key.into(), values);
        self
    }

    /// Replaces all values for an attribute on this document.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// let doc = doc.set_attr("reference", "Genesis 3:15");
    /// ```
    pub fn set_attr(self, _key: impl Into<String>, _values: impl IntoValues) -> Self {
        todo!()
    }

    /// Removes an attribute and all its values.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// let doc = doc.with_attr("reference", "Genesis 3:15");
    /// let doc = doc.remove_attr("reference");
    /// ```
    pub fn remove_attr(self, _key: &str) -> Self {
        todo!()
    }

    /// Adds a layer and returns the document. Panics if the layer ID is already in use.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// let doc = doc.with_layer("verses");
    /// ```
    pub fn with_layer(mut self, layer: impl Into<Layer>) -> Self {
        self.add_layer(layer).expect("layer ID is already in use");
        self
    }

    /// Adds a layer, returning an error if the layer ID is already in use.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// doc.add_layer("verses")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_layer(&mut self, layer: impl Into<Layer>) -> Result<(), Error> {
        let layer = layer.into();
        if self.layer(layer.id()).is_some() {
            return Err(Error::DuplicateLayer(layer.id().into()));
        }
        self.layers.push(layer);
        Ok(())
    }

    /// Appends NFC-normalized text and returns its byte range in the document.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// let range = doc.push("And I will put enmity")?;
    /// assert_eq!(range, 0..21);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn push(&mut self, text: &str) -> Result<Range<u32>, Error> {
        let text: String = text.nfc().collect();
        if !text.is_empty() && !self.text.is_empty() {
            let boundary = self
                .text
                .char_indices()
                .rev()
                .find(|(_, ch)| unicode_normalization::char::canonical_combining_class(*ch) == 0)
                .map_or(0, |(index, _)| index);
            let joined = self.text[boundary..].chars().chain(text.chars());
            if !joined.clone().nfc().eq(joined) {
                return Err(Error::SplitsCharacter);
            }
        }
        let start = self.len();
        let added = u32::try_from(text.len()).map_err(|_| Error::TooLong)?;
        let end = start.checked_add(added).ok_or(Error::TooLong)?;
        Arc::make_mut(&mut self.text).push_str(&text);
        Ok(start..end)
    }

    /// Appends text and adds a span over it in the given layer. The closure can set the span's ID and attributes.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// doc.add_layer("words")?;
    /// let range = doc.push_span("words", "enmity", |span| span.with_id("enmity"))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn push_span(
        &mut self,
        layer: &str,
        text: &str,
        build: impl FnOnce(Span) -> Span,
    ) -> Result<Range<u32>, Error> {
        let stored = self
            .layer(layer)
            .ok_or_else(|| Error::UnknownLayer(layer.into()))?;
        if stored.is_source() {
            return Err(Error::ReadOnlyLayer(layer.into()));
        }
        let start = self.len();
        let range = self.push(text)?;
        if let Err(error) = self.add_span(layer, build(Span::from(range.clone()))) {
            Arc::make_mut(&mut self.text).truncate(start as usize);
            return Err(error);
        }
        Ok(range)
    }

    /// Adds a span to the given layer and returns its reference, assigning an ID if needed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// doc.add_layer("verses")?;
    /// let range = doc.push("And I will put enmity")?;
    /// let verse = doc.add_span("verses", Span::from(range).with_id("GEN 3:15"))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_span(&mut self, layer: &str, mut span: Span) -> Result<SpanRef, Error> {
        let index = self
            .layers
            .iter()
            .position(|stored| stored.id() == layer)
            .ok_or_else(|| Error::UnknownLayer(layer.into()))?;
        if self.layers[index].is_source() {
            return Err(Error::ReadOnlyLayer(layer.into()));
        }
        match span.target() {
            Target::Text(ranges) => {
                for range in ranges {
                    if range.start > range.end || range.end > self.len() {
                        return Err(Error::OutOfRange(range.clone()));
                    }
                    for position in [range.start, range.end] {
                        if !self.text.is_char_boundary(position as usize) {
                            return Err(Error::NotCharBoundary(position));
                        }
                    }
                }
            }
            Target::Span(reference) => {
                if self.span(reference).is_none() {
                    return Err(Error::UnknownSpan(reference.clone()));
                }
            }
        }
        let ranges = match span.target() {
            Target::Text(ranges) => ranges.clone(),
            Target::Span(reference) => self.ranges(reference).unwrap().to_vec(),
        };
        let stored = &mut self.layers[index];
        let id = match span.id() {
            Some(id) => id.clone(),
            None => SpanId::Int(stored.next_id.expect("integer span IDs exhausted")),
        };
        let reference = SpanRef::new(layer, id.clone());
        if stored.span_slots.contains_key(&id) {
            return Err(Error::DuplicateId(reference));
        }
        if let SpanId::Int(value) = &id {
            if stored.next_id.is_some_and(|next| *value >= next) {
                stored.next_id = value.checked_add(1);
            }
        }
        span.id = Some(id.clone());
        stored.span_slots.insert(id, stored.spans.len());
        stored.spans.push(span);
        stored.resolved_ranges.push(ranges);
        Ok(reference)
    }

    /// Replaces the span identified by the reference.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// doc.replace_span(&verse, Span::from(range).with_attr("book", "Genesis"))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn replace_span(&mut self, _reference: &SpanRef, _span: Span) -> Result<(), Error> {
        todo!()
    }

    /// Removes the span identified by the reference.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// doc.remove_span(&verse)?; // The verse text remains in the document.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn remove_span(&mut self, _reference: &SpanRef) -> Result<(), Error> {
        todo!()
    }

    /// Resolves a range, span reference, or other selection input into document byte ranges.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, select_span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let selection = doc.get_selection(select_span("verses", "GEN 3:15"))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_selection(&self, _selection: impl IntoSelection) -> Result<Selection, Error> {
        todo!()
    }

    /// Returns a passage for the selection, or an error unless it resolves to exactly one range.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, select_span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let passage = doc.get_passage(select_span("verses", "GEN 3:15"))?;
    /// assert_eq!(passage.text(), "And I will put enmity");
    /// assert!(doc.get_passage([0..3, 15..21]).is_err()); // Two ranges, not one.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_passage(&self, _selection: impl IntoSelection) -> Result<Passage, Error> {
        todo!()
    }

    /// Returns one passage per resolved range, in document order.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// let passages = doc.get_passages([0..3, 15..21])?;
    /// assert_eq!(passages.len(), 2);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_passages(&self, _selection: impl IntoSelection) -> Result<Vec<Passage>, Error> {
        todo!()
    }

    /// Starts a query with the whole document selected.
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
    pub fn find(&self) -> Query<'_> {
        todo!()
    }

    /// Returns the document identifier.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// assert_eq!(doc.id(), "GEN 3:15");
    /// ```
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the length of the document text in bytes.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// assert_eq!(doc.len(), 21);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn len(&self) -> u32 {
        self.text.len() as u32
    }

    /// Returns whether the document contains no text.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// assert!(doc.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Returns the full document text.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// assert_eq!(doc.text(), "And I will put enmity");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the text at the given byte range.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// assert_eq!(doc.slice(15..21), "enmity");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn slice(&self, range: Range<u32>) -> &str {
        &self.text[range.start as usize..range.end as usize]
    }

    /// Returns the first value for the given attribute key, if present.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Value};
    /// # let mut doc = Document::new("GEN 3:15");
    /// let doc = doc.with_attr("book", "Genesis");
    /// assert_eq!(doc.attr("book"), Some(&Value::from("Genesis")));
    /// ```
    pub fn attr(&self, _key: &str) -> Option<&Value> {
        todo!()
    }

    /// Iterates over attribute key-value pairs.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// let doc = doc.with_attr("book", "Genesis");
    /// for (key, value) in doc.attrs() {
    ///     println!("{key}: {value:?}");
    /// }
    /// ```
    pub fn attrs(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.attrs.iter()
    }

    /// Iterates over all values for the given attribute key.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Document;
    /// # let mut doc = Document::new("GEN 3:15");
    /// let doc = doc.with_attr("themes", ["enmity", "offspring"]);
    /// assert_eq!(doc.values("themes").count(), 2);
    /// ```
    pub fn values(&self, _key: &str) -> impl Iterator<Item = &Value> {
        pending_iter()
    }

    /// Looks up a layer by its identifier.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// assert!(doc.layer("verses").is_some());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id() == id)
    }

    /// Iterates over the document's layers.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Layer, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let ids: Vec<_> = doc.layers().map(Layer::id).collect();
    /// assert_eq!(ids, ["verses"]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.layers.iter()
    }

    /// Looks up a span by its layer and ID.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let annotation = doc.span(&verse).unwrap();
    /// assert_eq!(annotation.id(), Some(verse.id()));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn span(&self, reference: &SpanRef) -> Option<&Span> {
        let layer = self.layer(reference.layer())?;
        let slot = layer.span_slots.get(reference.id())?;
        layer.spans.get(*slot)
    }

    /// Returns the document byte ranges a span targets, following any span references.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// assert_eq!(doc.ranges(&verse), Some(&[range][..]));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn ranges(&self, reference: &SpanRef) -> Option<&[Range<u32>]> {
        let layer = self.layer(reference.layer())?;
        let slot = layer.span_slots.get(reference.id())?;
        layer.resolved_ranges.get(*slot).map(Vec::as_slice)
    }
}

/// A value that can be converted into a [`Document`].
pub trait IntoDocument {
    /// The error returned when conversion fails.
    type Error: From<Error>;

    /// Converts this value into a document.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, IntoDocument};
    /// let excerpt = Document::new("GEN 3:15");
    /// let doc = excerpt.into_document()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn into_document(self) -> Result<Document, Self::Error>;
}

impl IntoDocument for Document {
    type Error = Error;

    fn into_document(self) -> Result<Document, Self::Error> {
        Ok(self)
    }
}

/// An annotation that targets one or more text ranges or another span. It can carry an ID and attributes.
/// Unlike a [`Selection`], a span is an annotation that can be stored in a layer, not just a set of text positions.
#[derive(Clone, Debug)]
pub struct Span {
    id: Option<SpanId>,
    target: Target,
    attrs: Attributes,
}

impl Span {
    /// Creates a span targeting the span with the given layer and ID.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Span;
    /// let note = Span::on("verses", "GEN 3:15").with_attr("topic", "enmity"); // The note points to the verse annotation, not a byte range.
    /// ```
    pub fn on(_layer: impl Into<String>, _id: impl Into<SpanId>) -> Self {
        todo!()
    }

    /// Creates a span targeting the referenced span.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Span, select_span};
    /// let verse = select_span("verses", "GEN 3:15");
    /// let note = Span::on_ref(&verse).with_attr("topic", "enmity"); // The new note points to the same verse.
    /// ```
    pub fn on_ref(_reference: &SpanRef) -> Self {
        todo!()
    }

    /// Sets the ID of this span.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Span;
    /// let word = Span::from(15..21).with_id("enmity");
    /// ```
    pub fn with_id(mut self, id: impl Into<SpanId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Appends values to an attribute on this span.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Span;
    /// let word = Span::from(15..21).with_attr("pos", "noun");
    /// ```
    pub fn with_attr(mut self, key: impl Into<String>, values: impl IntoValues) -> Self {
        self.attrs.append(key.into(), values);
        self
    }

    /// Replaces all values for an attribute on this span.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Span;
    /// let word = Span::from(15..21).with_attr("pos", "verb");
    /// let word = word.set_attr("pos", "noun");
    /// ```
    pub fn set_attr(self, _key: impl Into<String>, _values: impl IntoValues) -> Self {
        todo!()
    }

    /// Removes an attribute and all its values.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Span;
    /// let word = Span::from(15..21).with_attr("pos", "noun");
    /// let word = word.remove_attr("pos");
    /// ```
    pub fn remove_attr(self, _key: &str) -> Self {
        todo!()
    }

    /// Returns the span's ID, if it has one.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Span, SpanId};
    /// let word = Span::from(15..21).with_id("enmity");
    /// assert_eq!(word.id(), Some(&SpanId::from("enmity")));
    /// ```
    pub fn id(&self) -> Option<&SpanId> {
        self.id.as_ref()
    }

    /// Returns the text ranges or span reference that this annotation targets.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Span, Target};
    /// let note = Span::on("verses", "GEN 3:15");
    /// assert!(matches!(note.target(), Target::Span(_)));
    /// ```
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// Returns the first value for the given attribute key, if present.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Span, Value};
    /// let word = Span::from(15..21).with_attr("pos", "noun");
    /// assert_eq!(word.attr("pos"), Some(&Value::from("noun")));
    /// ```
    pub fn attr(&self, key: &str) -> Option<&Value> {
        self.attrs.values(key).next()
    }

    /// Iterates over all values for the given attribute key.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Span;
    /// let verse = Span::from(0..21).with_attr("themes", ["enmity", "offspring"]);
    /// assert_eq!(verse.values("themes").count(), 2);
    /// ```
    pub fn values(&self, _key: &str) -> impl Iterator<Item = &Value> {
        pending_iter()
    }

    /// Iterates over attribute key-value pairs.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Span;
    /// let word = Span::from(15..21).with_attr("pos", "noun");
    /// assert_eq!(word.attrs().next().unwrap().0, "pos");
    /// ```
    pub fn attrs(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.attrs.iter()
    }
}

/// Creates a span targeting one document byte range.
impl From<Range<u32>> for Span {
    fn from(range: Range<u32>) -> Self {
        Self::from(vec![range])
    }
}

/// Creates a span targeting a vector of document byte ranges.
impl From<Vec<Range<u32>>> for Span {
    fn from(mut ranges: Vec<Range<u32>>) -> Self {
        ranges.sort_by_key(|range| (range.start, range.end));
        let mut merged: Vec<Range<u32>> = Vec::with_capacity(ranges.len());
        for range in ranges {
            if let Some(last) = merged.last_mut() {
                if last.start <= last.end && range.start <= range.end && range.start <= last.end {
                    last.end = last.end.max(range.end);
                    continue;
                }
            }
            merged.push(range);
        }
        Self {
            id: None,
            target: Target::Text(merged),
            attrs: Attributes::default(),
        }
    }
}

/// Creates a span targeting an array of document byte ranges.
impl<const N: usize> From<[Range<u32>; N]> for Span {
    fn from(ranges: [Range<u32>; N]) -> Self {
        Self::from(Vec::from(ranges))
    }
}

/// What a [`Span`] annotates: document byte ranges or another span.
#[derive(Clone, Debug)]
pub enum Target {
    Text(Vec<Range<u32>>),
    Span(SpanRef),
}

/// An integer or string that identifies a span within its layer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpanId {
    Int(u64),
    Str(String),
}

impl From<u64> for SpanId {
    fn from(id: u64) -> Self {
        Self::Int(id)
    }
}

impl From<&str> for SpanId {
    fn from(id: &str) -> Self {
        Self::Str(id.into())
    }
}

/// A reference to a span by layer and span ID. It does not hold the annotation or its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanRef {
    layer: String,
    id: SpanId,
}

impl SpanRef {
    /// Creates a reference from a layer identifier and a span ID.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::SpanRef;
    /// let verse = SpanRef::new("verses", "GEN 3:15");
    /// ```
    pub fn new(layer: impl Into<String>, id: impl Into<SpanId>) -> Self {
        Self {
            layer: layer.into(),
            id: id.into(),
        }
    }

    /// Returns the identifier of the referenced span's layer.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::SpanRef;
    /// let verse = SpanRef::new("verses", "GEN 3:15");
    /// assert_eq!(verse.layer(), "verses");
    /// ```
    pub fn layer(&self) -> &str {
        &self.layer
    }

    /// Returns the referenced span's ID.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{SpanId, SpanRef};
    /// let verse = SpanRef::new("verses", "GEN 3:15");
    /// assert_eq!(verse.id(), &SpanId::from("GEN 3:15"));
    /// ```
    pub fn id(&self) -> &SpanId {
        &self.id
    }
}

/// A value stored under an attribute key on a document, layer, or span.
#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Int(integer), Self::Float(float)) | (Self::Float(float), Self::Int(integer)) => {
                *float >= i64::MIN as f64
                    && *float < -(i64::MIN as f64)
                    && *float == *integer as f64
                    && *float as i64 == *integer
            }
            (Self::Str(a), Self::Str(b)) => a == b,
            _ => false,
        }
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Str(value.into())
    }
}

macro_rules! value_from {
    ($($source:ty => $variant:ident),*) => {$(
        impl From<$source> for Value {
            fn from(value: $source) -> Self {
                Self::$variant(value.into())
            }
        }
    )*};
}

value_from!(String => Str, i32 => Int, i64 => Int, u32 => Int, f64 => Float, bool => Bool);

/// One or more attribute values: a value, or an array or `Vec` of values.
pub trait IntoValues {
    /// Converts the input into an owned list of attribute values.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::IntoValues;
    /// let themes = ["enmity", "offspring"].into_values();
    /// assert_eq!(themes.len(), 2);
    /// ```
    fn into_values(self) -> Vec<Value>;
}

macro_rules! into_values {
    ($($source:ty),*) => {$(
        impl IntoValues for $source {
            fn into_values(self) -> Vec<Value> {
                vec![self.into()]
            }
        }
    )*};
}

into_values!(Value, &str, String, i32, i64, u32, f64, bool);

impl<T: Into<Value>, const N: usize> IntoValues for [T; N] {
    fn into_values(self) -> Vec<Value> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<T: Into<Value>> IntoValues for Vec<T> {
    fn into_values(self) -> Vec<Value> {
        self.into_iter().map(Into::into).collect()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Attributes(Vec<(String, Vec<Value>)>);

impl Attributes {
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0
            .iter()
            .flat_map(|(key, values)| values.iter().map(move |value| (key.as_str(), value)))
    }

    pub(crate) fn append(&mut self, key: String, values: impl IntoValues) {
        let values = values.into_values();
        if values.is_empty() {
            return;
        }
        if let Some((_, stored)) = self.0.iter_mut().find(|(stored, _)| *stored == key) {
            stored.extend(values);
        } else {
            self.0.push((key, values));
        }
    }

    pub(crate) fn values(&self, key: &str) -> impl Iterator<Item = &Value> {
        self.0
            .iter()
            .find(|(stored, _)| stored == key)
            .into_iter()
            .flat_map(|(_, values)| values)
    }
}

/// An error from editing a document, resolving a selection, or running a query.
#[derive(Debug)]
pub enum Error {
    UnknownLayer(String),
    DuplicateLayer(String),
    UnknownSpan(SpanRef),
    DuplicateId(SpanRef),
    Cycle(SpanRef),
    ReadOnlyLayer(String),
    OutOfRange(Range<u32>),
    NotCharBoundary(u32),
    SplitsCharacter,
    TooLong,
    NotOne { found: usize },
    Source { layer: String, error: SourceError },
}

impl fmt::Display for Error {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl core::error::Error for Error {}
