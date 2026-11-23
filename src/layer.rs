use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};
use core::{cell::RefCell, fmt, ops::Range};

use crate::document::{Attributes, IntoValues, Span, SpanId, SpanRef, Target, Value};
use crate::source::SpanSource;

mod storage;

pub(crate) use storage::IdRef;
use storage::{AttributeColumns, Ids, Rows, SpanCache, Targets};
/// A named collection of spans, optionally populated by a read-only span source.
pub struct Layer {
    id: String,
    name: String,
    ids: Ids,
    pub(crate) next_id: Option<u64>,
    // Flat pairs of u32 endpoints, stored as Range<u32> so ranges() can
    // return a borrowed slice without a conversion or a second allocation.
    // A row is the original target for text spans, and a cache only for references.
    ranges: Rows<Range<u32>>,
    targets: Targets,
    attributes: AttributeColumns,
    borrowed_spans: SpanCache,
    pub(crate) point_span_count: usize,
    // Reverse edges for explicit targets only; same-ID edges can be inferred.
    pub(crate) dependents: BTreeMap<usize, Vec<(usize, usize)>>,
    free_slots: Vec<usize>,
    slot_count: usize,
    attribute_index: RefCell<Option<AttributeIndex>>,
    position_index: RefCell<Option<PositionIndex>>,
    pub(crate) source: Option<Box<dyn SpanSource>>,
    attrs: Attributes,
}

type AttributeIndex = BTreeMap<(u32, u32), Vec<u32>>;

struct PositionEntry {
    start: u32,
    end: u32,
    slot: u32,
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
            .slots()
            .flat_map(|slot| {
                layer.ranges(slot).iter().map(move |range| PositionEntry {
                    start: range.start,
                    end: range.end,
                    slot: slot as u32,
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
            slots.push(entry.slot as usize);
        }
        Self::lookup(&entries[mid + 1..], range, include_boundaries, slots);
    }
}

impl fmt::Debug for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Layer")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("span_count", &self.span_count())
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
            ids: Ids::default(),
            next_id: Some(1),
            ranges: Rows::default(),
            targets: Targets::default(),
            attributes: AttributeColumns::default(),
            borrowed_spans: SpanCache::default(),
            point_span_count: 0,
            dependents: BTreeMap::new(),
            free_slots: Vec::new(),
            slot_count: 0,
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
        Spans {
            layer: self,
            slots: None,
        }
    }

    pub(crate) fn span_count(&self) -> usize {
        self.ids.len()
    }

    pub(crate) fn slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.ids.slots()
    }

    pub(crate) fn slot(&self, id: IdRef<'_>) -> Option<usize> {
        self.ids.slot(id)
    }

    pub(crate) fn id_at(&self, slot: usize) -> IdRef<'_> {
        self.ids.id(slot)
    }

    pub(crate) fn ranges(&self, slot: usize) -> &[Range<u32>] {
        self.ranges.get(slot)
    }

    pub(crate) fn implicit_target_layer(&self) -> Option<&str> {
        self.targets.implicit_layer()
    }

    pub(crate) fn has_implicit_target(&self, slot: usize) -> bool {
        self.targets.is_implicit(slot)
    }

    pub(crate) fn explicit_target(&self, slot: usize) -> Option<&SpanRef> {
        self.targets.explicit(slot)
    }

    pub(crate) fn target_at(&self, slot: usize) -> Option<(&str, IdRef<'_>)> {
        if self.has_implicit_target(slot) {
            Some((self.implicit_target_layer().unwrap(), self.id_at(slot)))
        } else {
            self.explicit_target(slot)
                .map(|r| (r.layer(), r.id().into()))
        }
    }

    pub(crate) fn materialize(&self, slot: usize) -> Span {
        let target = self.target_at(slot).map_or_else(
            || Target::Text(self.ranges(slot).to_vec()),
            |(layer, id)| Target::Span(SpanRef::new(layer, SpanId::from(id))),
        );
        Span {
            id: Some(self.id_at(slot).into()),
            target,
            attrs: Attributes::from_pairs(self.attributes.attrs(slot)),
        }
    }

    pub(crate) fn borrowed_span(&self, slot: usize) -> &Span {
        self.borrowed_spans
            .get_or_init(slot, || self.materialize(slot))
    }

    pub(crate) fn attr_key(&self, key: &str) -> Option<u32> {
        self.attributes.key_code(key)
    }

    pub(crate) fn values(&self, slot: usize, key: u32) -> impl Iterator<Item = &Value> {
        self.attributes.values(slot, key)
    }

    pub(crate) fn insert_span(
        &mut self,
        id: SpanId,
        span: Span,
        resolved: Option<&[Range<u32>]>,
    ) -> usize {
        if let SpanId::Int(value) = id
            && self.next_id.is_some_and(|next| value >= next)
        {
            self.next_id = value.checked_add(1);
        }
        let slot = self.free_slots.pop().unwrap_or(self.slot_count);
        let _ = u32::try_from(slot).expect("layer exceeds u32::MAX slots");
        self.write_span(slot, (&id).into(), span, resolved, false);
        self.ids.insert(id, slot);
        if slot == self.slot_count {
            self.slot_count += 1;
        }
        slot
    }

    pub(crate) fn replace_span(
        &mut self,
        slot: usize,
        id: &SpanId,
        span: Span,
        resolved: Option<&[Range<u32>]>,
    ) {
        self.borrowed_spans.remove(slot);
        self.write_span(slot, id.into(), span, resolved, true);
    }

    fn write_span(
        &mut self,
        slot: usize,
        id: IdRef<'_>,
        span: Span,
        resolved: Option<&[Range<u32>]>,
        replacing: bool,
    ) {
        let target = match span.target {
            Target::Text(ranges) => {
                self.write_ranges(slot, ranges.into_iter(), replacing);
                None
            }
            Target::Span(reference) => {
                self.write_ranges(slot, resolved.unwrap().iter().cloned(), replacing);
                Some(reference)
            }
        };
        self.targets.set(slot, id, self.ids.len() == 0, target);
        self.attributes.set(slot, span.attrs.into_groups());
        self.attribute_index.get_mut().take();
    }

    pub(crate) fn refresh_ranges(&mut self, slot: usize, ranges: &[Range<u32>]) {
        self.write_ranges(slot, ranges.iter().cloned(), true);
    }

    fn write_ranges(
        &mut self,
        slot: usize,
        ranges: impl ExactSizeIterator<Item = Range<u32>>,
        replacing: bool,
    ) {
        if replacing {
            self.point_span_count -= usize::from(self.ranges(slot).iter().any(Range::is_empty));
        }
        self.ranges.set(slot, ranges);
        self.point_span_count += usize::from(self.ranges(slot).iter().any(Range::is_empty));
        self.position_index.get_mut().take();
    }

    pub(crate) fn remove_span(&mut self, slot: usize) {
        self.point_span_count -= usize::from(self.ranges(slot).iter().any(Range::is_empty));
        self.ids.remove(slot, &mut self.next_id);
        self.targets.remove(slot);
        self.borrowed_spans.remove(slot);
        self.attribute_index.get_mut().take();
        self.position_index.get_mut().take();
        // Leave the packed rows in place for reuse; only live IDs expose them.
        self.free_slots.push(slot);
    }

    pub(crate) fn slots_in_position_order(&self) -> Vec<usize> {
        let mut slots: Vec<usize> = self.slots().collect();
        slots.sort_by_key(|&slot| {
            self.ranges(slot)
                .first()
                .map_or(u32::MAX, |range| range.start)
        });
        slots
    }

    pub(crate) fn span_source(&self) -> Option<&dyn SpanSource> {
        self.source.as_deref()
    }

    pub(crate) fn touching_slots(&self, range: &Range<u32>) -> Vec<usize> {
        let mut slots = self.position_slots(range, false);
        // Match spans() ordering, including ID order for equal starts.
        slots.sort_unstable_by(|&a, &b| {
            self.ranges(a)[0]
                .start
                .cmp(&self.ranges(b)[0].start)
                .then_with(|| self.ids.compare(a, b))
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
        let Some(key) = self.attr_key(key) else {
            return 0;
        };
        let index = self.attribute_index.borrow();
        let Some(index) = index.as_ref() else {
            return self.span_count();
        };
        self.attributes
            .equal_codes(value)
            .filter_map(|value| index.get(&(key, value)))
            .map(Vec::len)
            .sum()
    }

    pub(crate) fn matching_slots(&self, key: &str, value: &Value) -> Vec<usize> {
        let Some(key) = self.attr_key(key) else {
            return Vec::new();
        };
        let mut index = self.attribute_index.borrow_mut();
        let index = index.get_or_insert_with(|| {
            let mut built = AttributeIndex::new();
            for slot in self.slots() {
                for &pair in self.attributes.codes(slot) {
                    let matching = built.entry(pair).or_default();
                    if matching.last() != Some(&(slot as u32)) {
                        matching.push(slot as u32);
                    }
                }
            }
            built
        });

        let mut slots = Vec::new();
        for code in self.attributes.equal_codes(value) {
            if let Some(matches) = index.get(&(key, code)) {
                slots.extend(matches.iter().map(|&slot| slot as usize));
            }
        }
        slots.sort_unstable();
        slots.dedup();
        slots
    }
}

struct Spans<'a> {
    layer: &'a Layer,
    slots: Option<alloc::vec::IntoIter<usize>>,
}

impl<'a> Iterator for Spans<'a> {
    type Item = &'a Span;

    fn next(&mut self) -> Option<Self::Item> {
        self.slots
            .get_or_insert_with(|| self.layer.slots_in_position_order().into_iter())
            .next()
            .map(|slot| self.layer.borrowed_span(slot))
    }

    fn count(self) -> usize {
        self.size_hint().0
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self
            .slots
            .as_ref()
            .map_or_else(|| self.layer.span_count(), ExactSizeIterator::len);
        (len, Some(len))
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
                        .slots()
                        .filter(|&slot| layer.ranges(slot).iter().any(Range::is_empty))
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
