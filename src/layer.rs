use alloc::{
    boxed::Box,
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
use core::{cell::RefCell, fmt, ops::Range};

use crate::document::{Attributes, IntoValues, Span, SpanId, Value};
use crate::source::SpanSource;

/// A named collection of spans, optionally populated by a read-only span source.
pub struct Layer {
    id: String,
    name: String,
    pub(crate) spans: Vec<Option<Span>>,
    pub(crate) span_slots: BTreeMap<SpanId, usize>,
    pub(crate) next_id: Option<u64>,
    pub(crate) resolved_ranges: Vec<Vec<Range<u32>>>,
    pub(crate) point_span_count: usize,
    // Slot addresses (layer index, slot), allocated only through the last targeted slot.
    pub(crate) dependents: Vec<Vec<(usize, usize)>>,
    pub(crate) free_slots: Vec<usize>,
    attribute_index: RefCell<Option<AttributeIndex>>,
    position_index: RefCell<Option<PositionIndex>>,
    pub(crate) source: Option<Box<dyn SpanSource>>,
    attrs: Attributes,
}

type AttributeIndex = BTreeMap<String, BTreeMap<IndexedValue, Vec<usize>>>;

struct PositionEntry {
    start: u32,
    end: u32,
    slot: usize,
    max_end: u32,
}

// A balanced interval tree in start order. Each slice's midpoint stores the
// maximum end of that subtree, allowing lookups to skip unrelated intervals
// even when a long enclosing span precedes many short ones.
struct PositionIndex {
    entries: Vec<PositionEntry>,
}

impl PositionIndex {
    fn build(layer: &Layer) -> Self {
        let mut entries: Vec<_> = layer
            .span_slots
            .values()
            .flat_map(|&slot| {
                layer.resolved_ranges[slot]
                    .iter()
                    .map(move |range| PositionEntry {
                        start: range.start,
                        end: range.end,
                        slot,
                        max_end: range.end,
                    })
            })
            .collect();
        entries.sort_unstable_by_key(|entry| (entry.start, entry.end, entry.slot));
        Self::augment(&mut entries);
        Self { entries }
    }

    fn augment(entries: &mut [PositionEntry]) -> u32 {
        if entries.is_empty() {
            return 0;
        }
        let mid = entries.len() / 2;
        let (left, rest) = entries.split_at_mut(mid);
        let (entry, right) = rest.split_first_mut().unwrap();
        entry.max_end = entry.end.max(Self::augment(left)).max(Self::augment(right));
        entry.max_end
    }

    fn lookup(
        entries: &[PositionEntry],
        range: &Range<u32>,
        include_boundaries: bool,
        slots: &mut Vec<usize>,
    ) {
        if entries.is_empty() {
            return;
        }
        let mid = entries.len() / 2;
        let entry = &entries[mid];
        if entry.max_end < range.start {
            return;
        }
        Self::lookup(&entries[..mid], range, include_boundaries, slots);
        if entry.start > range.end {
            return;
        }
        let touches = if include_boundaries {
            entry.end >= range.start
        } else if entry.start == entry.end {
            range.start <= entry.start && entry.start <= range.end
        } else {
            range.start < range.end && entry.start < range.end && range.start < entry.end
        };
        if touches {
            slots.push(entry.slot);
        }
        Self::lookup(&entries[mid + 1..], range, include_boundaries, slots);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum IndexedValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    Str(String),
}

impl From<&Value> for IndexedValue {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(value) => Self::Bool(*value),
            Value::Int(value) => Self::Int(*value),
            Value::Float(value) => Self::Float(if *value == 0.0 {
                0.0_f64.to_bits()
            } else {
                value.to_bits()
            }),
            Value::Str(value) => Self::Str(value.clone()),
        }
    }
}

impl fmt::Debug for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Layer")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("spans", &self.spans)
            .field("source", &self.is_source())
            .field("attrs", &self.attrs)
            .finish()
    }
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
            point_span_count: 0,
            dependents: Vec::new(),
            free_slots: Vec::new(),
            attribute_index: RefCell::new(None),
            position_index: RefCell::new(None),
            source: None,
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
    /// #             vec![Span::from(15..21).with_id("highlight").with_attr("topic", "enmity")]
    /// #         } else {
    /// #             Vec::new()
    /// #         })
    /// #     }
    /// # }
    /// let highlights = Layer::source("highlights", EnmityHighlight);
    /// ```
    pub fn source(id: impl Into<String>, source: impl SpanSource + 'static) -> Self {
        let mut layer = Self::new(id);
        layer.source = Some(Box::new(source));
        layer
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
    /// #             vec![Span::from(15..21).with_id("highlight").with_attr("topic", "enmity")]
    /// #         } else {
    /// #             Vec::new()
    /// #         })
    /// #     }
    /// # }
    /// let highlights = Layer::source("highlights", EnmityHighlight);
    /// assert!(highlights.is_source());
    /// ```
    pub fn is_source(&self) -> bool {
        self.source.is_some()
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
        self.slots_in_position_order()
            .into_iter()
            .map(|slot| self.spans[slot].as_ref().unwrap())
    }

    pub(crate) fn slots_in_position_order(&self) -> Vec<usize> {
        let mut slots: Vec<usize> = self.span_slots.values().copied().collect();
        slots.sort_by_key(|&slot| {
            self.resolved_ranges[slot]
                .first()
                .map_or(u32::MAX, |range| range.start)
        });
        slots
    }

    pub(crate) fn span_source(&self) -> Option<&dyn SpanSource> {
        self.source.as_deref()
    }

    pub(crate) fn invalidate_attribute_index(&mut self) {
        self.attribute_index.get_mut().take();
    }

    pub(crate) fn invalidate_position_index(&mut self) {
        self.position_index.get_mut().take();
    }

    pub(crate) fn touching_slots(&self, range: &Range<u32>) -> Vec<usize> {
        let mut slots = self.position_slots(range, false);
        // Match spans() ordering, including ID order for equal starts.
        slots.sort_unstable_by(|&a, &b| {
            self.resolved_ranges[a][0]
                .start
                .cmp(&self.resolved_ranges[b][0].start)
                .then_with(|| {
                    self.spans[a]
                        .as_ref()
                        .unwrap()
                        .id()
                        .cmp(&self.spans[b].as_ref().unwrap().id())
                })
        });
        slots
    }

    pub(crate) fn position_slots(
        &self,
        range: &Range<u32>,
        include_boundaries: bool,
    ) -> Vec<usize> {
        let mut index = self.position_index.borrow_mut();
        let index = index.get_or_insert_with(|| PositionIndex::build(self));
        let mut slots = Vec::new();
        PositionIndex::lookup(&index.entries, range, include_boundaries, &mut slots);
        slots.sort_unstable();
        slots.dedup();
        slots
    }

    pub(crate) fn estimated_matching_count(&self, key: &str, value: &Value) -> usize {
        let index = self.attribute_index.borrow();
        let Some(index) = index.as_ref() else {
            return self.span_slots.len();
        };
        let Some(values) = index.get(key) else {
            return 0;
        };
        indexed_values_for(value)
            .iter()
            .filter_map(|value| values.get(value))
            .map(Vec::len)
            .sum()
    }

    pub(crate) fn matching_slots(&self, key: &str, value: &Value) -> Vec<usize> {
        let mut index = self.attribute_index.borrow_mut();
        if index.is_none() {
            let mut built = AttributeIndex::new();
            for (&slot, span) in self
                .span_slots
                .values()
                .map(|slot| (slot, self.spans[*slot].as_ref().unwrap()))
            {
                let mut seen = Vec::new();
                for (attr, attr_value) in span.attrs() {
                    let key = (attr.to_string(), IndexedValue::from(attr_value));
                    if !seen.contains(&key) {
                        seen.push(key);
                    }
                }
                for (attr, attr_value) in seen {
                    built
                        .entry(attr)
                        .or_default()
                        .entry(attr_value)
                        .or_default()
                        .push(slot);
                }
            }
            *index = Some(built);
        }

        let Some(values) = index.as_ref().and_then(|index| index.get(key)) else {
            return Vec::new();
        };
        let mut slots = Vec::new();
        for indexed_value in indexed_values_for(value) {
            if let Some(matches) = values.get(&indexed_value) {
                slots.extend_from_slice(matches);
            }
        }
        slots.sort_unstable();
        slots.dedup();
        slots
    }
}

fn indexed_values_for(value: &Value) -> Vec<IndexedValue> {
    match value {
        Value::Int(value) => {
            let mut keys = alloc::vec![IndexedValue::Int(*value)];
            let float = *value as f64;
            if float >= i64::MIN as f64 && float < -(i64::MIN as f64) && float as i64 == *value {
                keys.push(IndexedValue::Float(float.to_bits()));
            }
            keys
        }
        Value::Float(value) => {
            let mut keys = alloc::vec![IndexedValue::Float(if *value == 0.0 {
                0.0_f64.to_bits()
            } else {
                value.to_bits()
            })];
            if value.is_finite() && *value >= i64::MIN as f64 && *value < -(i64::MIN as f64) {
                let integer = *value as i64;
                if integer as f64 == *value {
                    keys.push(IndexedValue::Int(integer));
                }
            }
            keys
        }
        _ => alloc::vec![IndexedValue::from(value)],
    }
}

impl From<&str> for Layer {
    /// Creates an empty layer using the string as its identifier.
    fn from(id: &str) -> Self {
        Self::new(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    #[test]
    fn planning_estimates_reuse_indexes_without_building_them() {
        let mut doc = Document::new("test").with_layer("words");
        doc.push("word word").unwrap();
        doc.add_span("words", Span::from(0..4).with_attr("pos", "verb"))
            .unwrap();
        doc.add_span("words", Span::from(5..9).with_attr("pos", "noun"))
            .unwrap();
        let layer = doc.layer("words").unwrap();
        let verb = Value::from("verb");
        assert_eq!(layer.estimated_matching_count("pos", &verb), 2);
        assert!(layer.attribute_index.borrow().is_none());
        assert!(layer.position_index.borrow().is_none());
        assert_eq!(layer.matching_slots("pos", &verb).len(), 1);
        assert_eq!(layer.estimated_matching_count("pos", &verb), 1);
        assert_eq!(
            layer.estimated_matching_count("pos", &Value::from("absent")),
            0
        );
        assert!(layer.position_index.borrow().is_none());
    }

    #[test]
    fn point_statistics_follow_edits_and_target_cascades() {
        fn check(doc: &Document) {
            for layer in doc.layers() {
                assert_eq!(
                    layer.point_span_count,
                    layer
                        .span_slots
                        .values()
                        .filter(|&&slot| layer.resolved_ranges[slot].iter().any(Range::is_empty))
                        .count()
                );
            }
        }
        let mut doc = Document::new("test").with_layer("words").with_layer("tags");
        doc.push("word").unwrap();
        let word = doc.add_span("words", Span::from(0..4)).unwrap();
        doc.add_span("tags", Span::on_ref(&word)).unwrap();
        check(&doc);
        doc.replace_span(&word, Span::from(2..2)).unwrap();
        check(&doc);
        assert_eq!(doc.layer("tags").unwrap().point_span_count, 1);
        doc.replace_span(&word, Span::from(0..4)).unwrap();
        check(&doc);
        doc.replace_span(&word, Span::from(4..4)).unwrap();
        check(&doc);
        doc.remove_span(&word).unwrap();
        check(&doc);
        assert_eq!(doc.layer("tags").unwrap().point_span_count, 0);
        doc.add_span("words", Span::from(0..4)).unwrap();
        check(&doc);
    }

    #[test]
    fn position_index_is_lazy_and_only_invalidated_by_span_edits() {
        let mut doc = Document::new("test").with_layer("words").with_layer("tags");
        doc.push("word").unwrap();
        let word = doc.add_span("words", Span::from(0..4)).unwrap();
        doc.add_span("tags", Span::on_ref(&word)).unwrap();
        assert!(
            doc.layer("words")
                .unwrap()
                .position_index
                .borrow()
                .is_none()
        );
        doc.find().with_layers(["words"]).get_passage().unwrap();
        assert!(
            doc.layer("words")
                .unwrap()
                .position_index
                .borrow()
                .is_some()
        );
        assert!(doc.layer("tags").unwrap().position_index.borrow().is_none());
        doc.get_passage(..).unwrap();
        doc.push(" more").unwrap();
        assert!(
            doc.layer("words")
                .unwrap()
                .position_index
                .borrow()
                .is_some()
        );
        doc.replace_span(&word, Span::from(5..9)).unwrap();
        assert!(
            doc.layer("words")
                .unwrap()
                .position_index
                .borrow()
                .is_none()
        );
        assert!(doc.layer("tags").unwrap().position_index.borrow().is_none());
    }
}
