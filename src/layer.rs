use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::ops::Range;

use crate::document::{Attributes, IntoValues, Span, SpanId, Value};
use crate::source::SpanSource;

/// A named collection of spans, optionally populated by a read-only span source.
#[derive(Debug)]
pub struct Layer {
    id: String,
    name: String,
    pub(crate) spans: Vec<Span>,
    pub(crate) span_slots: BTreeMap<SpanId, usize>,
    pub(crate) next_id: Option<u64>,
    pub(crate) resolved_ranges: Vec<Vec<Range<u32>>>,
    attrs: Attributes,
}

impl Layer {
    /// Creates an empty, mutable layer with the given identifier.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Layer;
    /// let verses = Layer::new("verses");
    /// ```
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        assert!(!id.is_empty(), "layer ID must not be empty");
        Self {
            name: id.clone(),
            id,
            spans: Vec::new(),
            span_slots: BTreeMap::new(),
            next_id: Some(1),
            resolved_ranges: Vec::new(),
            attrs: Attributes::default(),
        }
    }

    /// Creates a layer whose spans are supplied by a `SpanSource`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Layer, SourceError, Span, SpanSource};
    /// # struct EnmityHighlight;
    /// # impl SpanSource for EnmityHighlight {
    /// #     fn get_spans(&self, range: std::ops::Range<u32>) -> Result<Vec<Span>, SourceError> {
    /// #         Ok(if range.start < 21 && 15 < range.end {
    /// #             vec![Span::from(15..21).with_attr("topic", "enmity")]
    /// #         } else {
    /// #             Vec::new()
    /// #         })
    /// #     }
    /// # }
    /// let highlights = Layer::source("highlights", EnmityHighlight);
    /// ```
    pub fn source(_id: impl Into<String>, _source: impl SpanSource + 'static) -> Self {
        todo!()
    }

    /// Sets the human-readable name of this layer.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Layer;
    /// let verses = Layer::new("verses").with_name("Bible verses");
    /// ```
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Adds attribute values to this layer.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Layer;
    /// let verses = Layer::new("verses").with_attr("book", "Genesis");
    /// ```
    pub fn with_attr(mut self, key: impl Into<String>, values: impl IntoValues) -> Self {
        self.attrs.append(key.into(), values);
        self
    }

    /// Iterates over this layer's attributes and values.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Layer;
    /// let verses = Layer::new("verses").with_attr("book", "Genesis");
    /// assert_eq!(verses.attrs().next().unwrap().0, "book");
    /// ```
    pub fn attrs(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.attrs.iter()
    }

    /// Returns this layer's unique identifier.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Layer;
    /// let verses = Layer::new("verses");
    /// assert_eq!(verses.id(), "verses");
    /// ```
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns this layer's display name.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::Layer;
    /// let verses = Layer::new("verses").with_name("Bible verses");
    /// assert_eq!(verses.name(), "Bible verses");
    /// ```
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether this layer obtains spans from a source.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Layer, SourceError, Span, SpanSource};
    /// # struct EnmityHighlight;
    /// # impl SpanSource for EnmityHighlight {
    /// #     fn get_spans(&self, range: std::ops::Range<u32>) -> Result<Vec<Span>, SourceError> {
    /// #         Ok(if range.start < 21 && 15 < range.end {
    /// #             vec![Span::from(15..21).with_attr("topic", "enmity")]
    /// #         } else {
    /// #             Vec::new()
    /// #         })
    /// #     }
    /// # }
    /// let highlights = Layer::source("highlights", EnmityHighlight);
    /// assert!(highlights.is_source());
    /// ```
    pub fn is_source(&self) -> bool {
        false
    }

    /// Iterates over the spans stored directly in this layer.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let verses = doc.layer("verses").unwrap();
    /// assert_eq!(verses.spans().count(), 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn spans(&self) -> impl Iterator<Item = &Span> {
        let mut slots: Vec<usize> = (0..self.spans.len()).collect();
        slots.sort_by_key(|&slot| {
            self.resolved_ranges[slot]
                .first()
                .map_or(u32::MAX, |range| range.start)
        });
        slots.into_iter().map(|slot| &self.spans[slot])
    }
}

impl From<&str> for Layer {
    /// Creates an empty layer using the string as its identifier.
    fn from(id: &str) -> Self {
        Self::new(id)
    }
}
