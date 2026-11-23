use alloc::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    string::String,
    sync::Arc,
    vec,
    vec::Vec,
};
use core::{fmt, ops::Range};
use unicode_normalization::UnicodeNormalization;

use crate::layer::{IdRef, Layer};
use crate::passage::Passage;
use crate::query::Query;
use crate::selection::{IntoSelection, Selection, SpanFilter, SpanView};
use crate::source::SourceError;

/// A document's UTF-8 text, metadata, and layers of annotations. Text is append-only so existing byte offsets stay valid.
#[derive(Clone, Debug)]
pub struct Document {
    id: String,
    text: Arc<String>,
    layers: Vec<Arc<Layer>>,
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
    pub fn set_attr(mut self, key: impl Into<String>, values: impl IntoValues) -> Self {
        self.attrs.set(key.into(), values);
        self
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
    pub fn remove_attr(mut self, key: &str) -> Self {
        self.attrs.remove(key);
        self
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
        self.layers.push(Arc::new(layer));
        Ok(())
    }

    /// Freezes all stored layers and builds their query indexes.
    ///
    /// Clones of the returned document share these layers. Add a new layer
    /// for spans that still need to be editable. Source layers stay live.
    pub fn freeze_layers(mut self) -> Self {
        for layer in &mut self.layers {
            if !layer.is_read_only() {
                Arc::make_mut(layer).freeze();
            }
        }
        self
    }

    /// Freezes a stored layer and builds its query indexes.
    ///
    /// A later edit in another layer cannot remove or change the ranges of
    /// a frozen dependent. Freezing an already read-only layer is a no-op.
    pub fn freeze_layer(&mut self, id: &str) -> Result<(), Error> {
        let layer = self
            .layers
            .iter_mut()
            .find(|layer| layer.id() == id)
            .ok_or_else(|| Error::UnknownLayer(id.into()))?;
        if !layer.is_read_only() {
            Arc::make_mut(layer).freeze();
        }
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
        if stored.is_read_only() {
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
    pub fn add_span(&mut self, layer: &str, span: Span) -> Result<SpanRef, Error> {
        let index = self
            .layers
            .iter()
            .position(|stored| stored.id() == layer)
            .ok_or_else(|| Error::UnknownLayer(layer.into()))?;
        if self.layers[index].is_read_only() {
            return Err(Error::ReadOnlyLayer(layer.into()));
        }
        let parents = self.validate_target(span.target())?;
        let stored = &self.layers[index];
        let id = span
            .id
            .clone()
            .unwrap_or_else(|| SpanId::Int(stored.next_id.expect("integer span IDs exhausted")));
        let reference = SpanRef::new(layer, id.clone());
        if stored.slot((&id).into()).is_some() {
            return Err(Error::DuplicateId(reference));
        }
        let resolved = resolve_target_ranges(parents, |loc| self.ranges_at(loc))?;
        let stored = Arc::make_mut(&mut self.layers[index]);
        let slot = stored.insert_span(id, span, resolved.as_ref().map(RangeSnapshot::as_slice));
        if !stored.has_implicit_target(slot) {
            for parent in target_links(parents) {
                self.attach_target(parent, (index, slot));
            }
        }
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
    pub fn replace_span(&mut self, reference: &SpanRef, span: Span) -> Result<(), Error> {
        let location @ (layer, slot) = self.edit_location(reference)?;
        let parents = self.validate_target(span.target())?;
        let (reachable, order) = self.dependent_order(location);
        if target_links(parents).any(|parent| reachable.contains(&parent)) {
            return Err(Error::Cycle(reference.clone()));
        }

        let resolved = resolve_target_ranges(parents, |loc| self.ranges_at(loc))?;
        // Work out all range changes before modifying any span. A replacement
        // can invalidate a downstream Between, or move both of its endpoints.
        let mut updates = BTreeMap::new();
        if !order.is_empty() {
            let replacement = match span.target() {
                Target::Text(ranges) => ranges.as_slice(),
                _ => resolved.as_ref().unwrap().as_slice(),
            };
            if replacement != self.ranges_at(location) {
                updates.insert(location, RangeSnapshot::new(replacement));
                for child in order {
                    let ranges = resolve_target_ranges(self.targets_at(child), |loc| {
                        updates
                            .get(&loc)
                            .map_or_else(|| self.ranges_at(loc), RangeSnapshot::as_slice)
                    })?
                    .unwrap();
                    if ranges.as_slice() != self.ranges_at(child) {
                        if self.layers[child.0].is_read_only() {
                            return Err(Error::ReadOnlyLayer(self.layers[child.0].id().into()));
                        }
                        updates.insert(child, ranges);
                    }
                }
                updates.remove(&location);
            }
        }

        self.detach_target(layer, slot);
        let stored = Arc::make_mut(&mut self.layers[layer]);
        stored.replace_span(
            slot,
            reference.id(),
            span,
            resolved.as_ref().map(RangeSnapshot::as_slice),
        );
        if !stored.has_implicit_target(slot) {
            for parent in target_links(parents) {
                self.attach_target(parent, location);
            }
        }
        for ((layer, slot), ranges) in updates {
            Arc::make_mut(&mut self.layers[layer]).refresh_ranges(slot, ranges.as_slice());
        }
        Ok(())
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
    pub fn remove_span(&mut self, reference: &SpanRef) -> Result<(), Error> {
        let location = self.edit_location(reference)?;
        let (removed, order) = self.dependent_order(location);
        for &(layer, _) in &order {
            if self.layers[layer].is_read_only() {
                return Err(Error::ReadOnlyLayer(self.layers[layer].id().into()));
            }
        }
        // A parent can have many children in the same layer. Prune each
        // reverse list once rather than scanning it for every removed child.
        let mut affected = BTreeSet::new();
        for (layer, slot) in order.into_iter().rev().chain(core::iter::once(location)) {
            if !self.layers[layer].has_implicit_target(slot) {
                affected.extend(
                    target_links(self.targets_at((layer, slot))).map(|parent| (layer, parent)),
                );
            }
            Arc::make_mut(&mut self.layers[layer]).remove_span(slot);
        }
        for (layer, parent) in affected {
            let dependents = &mut Arc::make_mut(&mut self.layers[layer]).dependents;
            let children = dependents.get_mut(&parent).unwrap();
            children.retain(|&slot| !removed.contains(&(layer, slot)));
            if children.is_empty() {
                dependents.remove(&parent);
            }
        }
        Ok(())
    }

    /// Resolves a range, span reference, or other selection input into document byte ranges.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, span_id};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let selection = doc.get_selection(span_id("verses", "GEN 3:15"))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_selection(&self, _selection: impl IntoSelection) -> Result<Selection, Error> {
        _selection.into_selection(self)
    }

    /// Returns a passage for the selection, or an error unless it resolves to exactly one range.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, span_id};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let passage = doc.get_passage(span_id("verses", "GEN 3:15"))?;
    /// assert_eq!(passage.text(), "And I will put enmity");
    /// assert!(doc.get_passage([0..3, 15..21]).is_err()); // Two ranges, not one.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_passage(&self, selection: impl IntoSelection) -> Result<Passage, Error> {
        let selection = selection.into_selection(self)?;
        if selection.ranges().len() != 1 {
            return Err(Error::NotOne {
                found: selection.ranges().len(),
            });
        }
        self.passage(selection.ranges()[0].clone(), None)
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
    pub fn get_passages(&self, selection: impl IntoSelection) -> Result<Vec<Passage>, Error> {
        let selection = selection.into_selection(self)?;
        selection
            .ranges()
            .iter()
            .cloned()
            .map(|range| self.passage(range, None))
            .collect()
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
        Query::new(self)
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
    pub fn attr(&self, key: &str) -> Option<&Value> {
        self.attrs.values(key).next()
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
    pub fn values(&self, key: &str) -> impl Iterator<Item = &Value> {
        self.attrs.values(key)
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
        self.layers().find(|layer| layer.id() == id)
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
        self.layers.iter().map(Arc::as_ref)
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
        let slot = layer.slot(reference.id().into())?;
        Some(layer.borrowed_span(slot))
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
        let slot = layer.slot(reference.id().into())?;
        Some(layer.ranges(slot))
    }

    pub(crate) fn selection_from_ranges(
        &self,
        ranges: Vec<Range<u32>>,
    ) -> Result<Selection, Error> {
        for range in &ranges {
            self.validate_range(range)?;
        }
        Ok(Selection::from_ranges(ranges))
    }

    fn validate_range(&self, range: &Range<u32>) -> Result<(), Error> {
        if range.start > range.end || range.end > self.len() {
            return Err(Error::OutOfRange(range.clone()));
        }
        for position in [range.start, range.end] {
            if !self.text.is_char_boundary(position as usize) {
                return Err(Error::NotCharBoundary(position));
            }
        }
        Ok(())
    }

    pub(crate) fn selection_for_filter(&self, filter: &SpanFilter) -> Result<Selection, Error> {
        Ok(Selection::from_ranges(self.matching_ranges(filter)?))
    }

    pub(crate) fn estimate_filter_cost(&self, filter: &SpanFilter) -> usize {
        self.layers
            .iter()
            .filter(|layer| {
                filter
                    .layer_ids()
                    .is_none_or(|ids| ids.iter().any(|id| id == layer.id()))
            })
            .fold(0_usize, |cost, layer| {
                let candidates = if layer.is_source() {
                    // Sources have no local statistics. Do not read one to plan.
                    self.len() as usize
                } else if let Some((key, value)) = filter.index_hint() {
                    layer.estimated_matching_count(key, value)
                } else {
                    layer.span_count()
                };
                cost.saturating_add(candidates)
            })
    }

    pub(crate) fn filter_may_select_points(&self, filter: &SpanFilter) -> bool {
        self.layers.iter().any(|layer| {
            filter
                .layer_ids()
                .is_none_or(|ids| ids.iter().any(|id| id == layer.id()))
                && (layer.is_source() || layer.point_span_count > 0)
        })
    }

    pub(crate) fn selection_for_filter_within(
        &self,
        filter: &SpanFilter,
        hits: &Selection,
    ) -> Result<Selection, Error> {
        let indices = self.matching_layer_indices(filter)?;
        // Source results must still be validated, even when hits are empty.
        // Keeping the full source read also preserves its error behavior.
        if hits.ranges() == core::slice::from_ref(&(0..self.len())) {
            return self.selection_for_filter(filter);
        }
        if indices.iter().any(|&index| self.layers[index].is_source()) {
            return Ok(hits.intersect(&self.selection_for_filter(filter)?));
        }
        let covered: u64 = hits
            .ranges()
            .iter()
            .map(|r| u64::from(r.end - r.start) + 1)
            .sum();
        let mut ranges = Vec::new();
        for index in indices {
            let layer = &self.layers[index];
            let count = layer.span_count();
            if count == 0 || hits.is_empty() {
                continue;
            }
            let candidates = filter.index_hint().map_or(count, |(key, value)| {
                layer.estimated_matching_count(key, value)
            });
            // Estimate interval-tree probes plus the fraction of spans covered
            // by the hits. No index is built merely to produce this estimate.
            let position_cost = hits
                .ranges()
                .len()
                .saturating_mul(count.ilog2() as usize + 1)
                .saturating_add(
                    (covered.saturating_mul(count as u64) / (u64::from(self.len()) + 1))
                        .min(usize::MAX as u64) as usize,
                );
            // Include boundary-adjacent text ranges as well as touching ones.
            // They may absorb points during normalization before intersection.
            let slots = if position_cost < candidates {
                let mut slots: Vec<_> = hits
                    .ranges()
                    .iter()
                    .flat_map(|range| layer.position_slots(range, true))
                    .collect();
                slots.sort_unstable();
                slots.dedup();
                slots
            } else if let Some((key, value)) = filter.index_hint() {
                layer.matching_slots(key, value)
            } else {
                layer.slots().collect()
            };
            for slot in slots {
                if filter.matches(self, SpanView::Stored(layer, slot), layer.id()) {
                    ranges.extend_from_slice(layer.ranges(slot));
                }
            }
        }
        Ok(hits.intersect(&Selection::from_ranges(ranges)))
    }

    pub(crate) fn matching_ranges(&self, filter: &SpanFilter) -> Result<Vec<Range<u32>>, Error> {
        let mut ranges = Vec::new();
        self.visit_matching_ranges(filter, |matched| ranges.extend_from_slice(matched))?;
        Ok(ranges)
    }

    pub(crate) fn count_matching_spans_touching(
        &self,
        filter: &SpanFilter,
        hits: &Selection,
    ) -> Result<usize, Error> {
        let mut count = 0;
        let full = hits.ranges() == core::slice::from_ref(&(0..self.len()));
        for index in self.matching_layer_indices(filter)? {
            let layer = &self.layers[index];
            if full && !filter.has_conditions() && !layer.is_source() {
                count += layer
                    .slots()
                    .filter(|&slot| !layer.ranges(slot).is_empty())
                    .count();
            } else {
                self.visit_matching_layer_ranges(layer, filter, |ranges| {
                    count += usize::from(if full {
                        !ranges.is_empty()
                    } else {
                        span_ranges_touch(ranges, hits)
                    });
                })?;
            }
        }
        Ok(count)
    }

    // Visit one slice per span. Stored ranges already resolved and validated at
    // insertion need not be cloned or re-resolved for range-only queries.
    pub(crate) fn visit_matching_ranges(
        &self,
        filter: &SpanFilter,
        mut visit: impl FnMut(&[Range<u32>]),
    ) -> Result<(), Error> {
        for index in self.matching_layer_indices(filter)? {
            self.visit_matching_layer_ranges(&self.layers[index], filter, &mut visit)?;
        }
        Ok(())
    }

    fn visit_matching_layer_ranges(
        &self,
        layer: &Layer,
        filter: &SpanFilter,
        mut visit: impl FnMut(&[Range<u32>]),
    ) -> Result<(), Error> {
        if layer.is_source() {
            for span in self.read_source_spans(layer, filter)? {
                if let Some(ranges) = self.matching_source_ranges(layer, filter, &span)? {
                    visit(&ranges);
                }
            }
            return Ok(());
        }
        if !filter.has_conditions() {
            for slot in layer.slots() {
                let ranges = layer.ranges(slot);
                if !ranges.is_empty() {
                    visit(ranges);
                }
            }
            return Ok(());
        }
        let visit_slot = |slot: usize| {
            if filter.matches(self, SpanView::Stored(layer, slot), layer.id()) {
                visit(layer.ranges(slot));
            }
        };
        if let Some((key, value)) = filter.index_hint() {
            layer
                .matching_slots(key, value)
                .into_iter()
                .for_each(visit_slot);
        } else {
            layer.slots().for_each(visit_slot);
        }
        Ok(())
    }

    fn read_source_spans(&self, layer: &Layer, filter: &SpanFilter) -> Result<Vec<Span>, Error> {
        let source = layer.span_source().unwrap();
        source
            .find_spans(filter)
            .unwrap_or_else(|| source.get_spans(0..self.len()))
            .map_err(|error| Error::Source {
                layer: layer.id().into(),
                error,
            })
    }

    fn matching_source_ranges<'a>(
        &'a self,
        layer: &Layer,
        filter: &SpanFilter,
        span: &'a Span,
    ) -> Result<Option<Cow<'a, [Range<u32>]>>, Error> {
        if !filter.matches(self, SpanView::Borrowed(span), layer.id()) || span.id().is_none() {
            return Ok(None);
        }
        let ranges = match span.target() {
            Target::Span(reference) => self.ranges(reference).map(Cow::Borrowed),
            Target::Between(first, last) => self
                .source_between_range(first, last)?
                .map(|range| Cow::Owned(vec![range])),
            Target::Text(ranges) if ranges.len() <= 1 => {
                if let Some(range) = ranges.first() {
                    self.validate_range(range)?;
                }
                Some(Cow::Borrowed(ranges.as_slice()))
            }
            Target::Text(ranges) => Some(Cow::Owned(
                self.selection_from_ranges(ranges.clone())?.into_ranges(),
            )),
        };
        Ok(ranges)
    }

    fn matching_layer_indices(&self, filter: &SpanFilter) -> Result<Vec<usize>, Error> {
        if let Some(layer_ids) = filter.layer_ids() {
            layer_ids
                .iter()
                .map(|id| {
                    self.layers
                        .iter()
                        .position(|layer| layer.id() == id)
                        .ok_or_else(|| Error::UnknownLayer(id.clone()))
                })
                .collect()
        } else {
            Ok((0..self.layers.len()).collect())
        }
    }

    pub(crate) fn matching_spans_touching(
        &self,
        filter: &SpanFilter,
        hits: &Selection,
    ) -> Result<Vec<MatchedSpan>, Error> {
        let mut matches = Vec::new();
        for layer_index in self.matching_layer_indices(filter)? {
            let layer = &self.layers[layer_index];
            if layer.is_source() {
                for span in self.read_source_spans(layer, filter)? {
                    if let Some(ranges) = self.matching_source_ranges(layer, filter, &span)?
                        && span_ranges_touch(&ranges, hits)
                    {
                        let ranges = ranges.into_owned();
                        matches.push(MatchedSpan {
                            layer_index,
                            span,
                            ranges,
                        });
                    }
                }
                continue;
            }
            let mut push_slot = |slot: usize| {
                if (!filter.has_conditions()
                    || filter.matches(self, SpanView::Stored(layer, slot), layer.id()))
                    && span_ranges_touch(layer.ranges(slot), hits)
                {
                    matches.push(MatchedSpan {
                        layer_index,
                        span: layer.materialize(slot),
                        ranges: layer.ranges(slot).to_vec(),
                    });
                }
            };
            if let Some((key, value)) = filter.index_hint() {
                layer
                    .matching_slots(key, value)
                    .into_iter()
                    .for_each(push_slot);
            } else if !filter.has_conditions() {
                // Final results are sorted by start. An ID-ordered scan keeps
                // the same tie order without sorting spans that won't be read.
                layer.slots().for_each(push_slot);
            } else {
                // Preserve ordering for materialized spans with tied starts.
                for slot in layer.slots_in_position_order() {
                    push_slot(slot);
                }
            }
        }
        Ok(matches)
    }

    // Visit stored span views for aggregation in position/ID order. Source
    // spans still use the usual filter pushdown, validation, and error handling.
    pub(crate) fn visit_layer_spans_touching(
        &self,
        layer_id: &str,
        hits: &Selection,
        mut visit: impl FnMut(SpanView<'_>),
    ) -> Result<(), Error> {
        let layer = self
            .layer(layer_id)
            .ok_or_else(|| Error::UnknownLayer(layer_id.into()))?;
        if layer.is_source() {
            let filter = crate::spans_in(layer_id);
            for span in self.read_source_spans(layer, &filter)? {
                if let Some(ranges) = self.matching_source_ranges(layer, &filter, &span)?
                    && span_ranges_touch(&ranges, hits)
                {
                    visit(SpanView::Borrowed(&span));
                }
            }
        } else {
            for slot in layer.slots_in_position_order() {
                if span_ranges_touch(layer.ranges(slot), hits) {
                    visit(SpanView::Stored(layer, slot));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn passage(
        &self,
        range: Range<u32>,
        included_layers: Option<&[String]>,
    ) -> Result<Passage, Error> {
        let selection = self.selection_from_ranges(vec![range.clone()])?;
        let range = selection.ranges()[0].clone();
        if let Some(layers) = included_layers {
            for layer in layers {
                if self.layer(layer).is_none() {
                    return Err(Error::UnknownLayer(layer.clone()));
                }
            }
        }

        let mut spans = Vec::new();
        let mut errors = Vec::new();
        for layer in &self.layers {
            if included_layers.is_some_and(|layers| !layers.iter().any(|id| id == layer.id())) {
                continue;
            }

            let layer_spans = if let Some(source) = layer.span_source() {
                match source.get_spans(range.clone()) {
                    Ok(spans) => spans,
                    Err(error) => {
                        errors.push((layer.id().into(), Arc::new(error)));
                        continue;
                    }
                }
            } else {
                let mut layer_passage_spans = Vec::new();
                for slot in layer.touching_slots(&range) {
                    if let Some(passage_span) = crate::passage::PassageSpan::new(
                        self.text.clone(),
                        range.clone(),
                        layer.id().into(),
                        layer.materialize(slot),
                        layer.ranges(slot),
                    ) {
                        layer_passage_spans.push(passage_span);
                    }
                }
                layer_passage_spans
                    .sort_by_key(|span| span.local().first().map_or(u32::MAX, |range| range.start));
                spans.extend(layer_passage_spans);
                continue;
            };

            let mut layer_passage_spans = Vec::new();
            for span in layer_spans {
                if layer.is_source() && span.id().is_none() {
                    continue;
                }
                let ranges = match span.target() {
                    Target::Text(ranges) => ranges.clone(),
                    Target::Span(reference) => match self.ranges(reference) {
                        Some(ranges) => ranges.to_vec(),
                        None => continue,
                    },
                    Target::Between(first, last) => match self.source_between_range(first, last)? {
                        Some(range) => vec![range],
                        None => continue,
                    },
                };
                let ranges = self.selection_from_ranges(ranges)?.ranges().to_vec();
                if let Some(passage_span) = crate::passage::PassageSpan::new(
                    self.text.clone(),
                    range.clone(),
                    layer.id().into(),
                    span,
                    &ranges,
                ) {
                    layer_passage_spans.push(passage_span);
                }
            }
            layer_passage_spans
                .sort_by_key(|span| span.local().first().map_or(u32::MAX, |range| range.start));
            spans.extend(layer_passage_spans);
        }

        Ok(Passage::new(self.text.clone(), range, spans, errors))
    }

    fn span_location(&self, reference: &SpanRef) -> Option<(usize, usize)> {
        self.span_location_by_id(reference.layer(), reference.id().into())
    }

    pub(crate) fn span_location_by_id(
        &self,
        layer_id: &str,
        id: IdRef<'_>,
    ) -> Option<(usize, usize)> {
        let layer = self
            .layers
            .iter()
            .position(|layer| layer.id() == layer_id)?;
        Some((layer, self.layers[layer].slot(id)?))
    }

    pub(crate) fn span_view(&self, layer_id: &str, id: IdRef<'_>) -> Option<SpanView<'_>> {
        let (layer, slot) = self.span_location_by_id(layer_id, id)?;
        Some(SpanView::Stored(&self.layers[layer], slot))
    }

    fn edit_location(&self, reference: &SpanRef) -> Result<(usize, usize), Error> {
        let layer = self
            .layers
            .iter()
            .position(|layer| layer.id() == reference.layer())
            .ok_or_else(|| Error::UnknownLayer(reference.layer().into()))?;
        if self.layers[layer].is_read_only() {
            return Err(Error::ReadOnlyLayer(reference.layer().into()));
        }
        let slot = self.layers[layer]
            .slot(reference.id().into())
            .ok_or_else(|| Error::UnknownSpan(reference.clone()))?;
        Ok((layer, slot))
    }

    fn validate_target(&self, target: &Target) -> Result<TargetLocations, Error> {
        let locate = |reference: &SpanRef| {
            self.span_location(reference)
                .ok_or_else(|| Error::UnknownSpan(reference.clone()))
        };
        match target {
            Target::Text(ranges) => {
                for range in ranges {
                    self.validate_range(range)?;
                }
                Ok([None, None])
            }
            Target::Span(reference) => Ok([Some(locate(reference)?), None]),
            Target::Between(first, last) => {
                validate_endpoint_layers(first, last)?;
                Ok([Some(locate(first)?), Some(locate(last)?)])
            }
        }
    }

    fn source_between_range(
        &self,
        first: &SpanRef,
        last: &SpanRef,
    ) -> Result<Option<Range<u32>>, Error> {
        let (Some(first_ranges), Some(last_ranges)) = (self.ranges(first), self.ranges(last))
        else {
            return Ok(None);
        };
        validate_endpoint_layers(first, last)?;
        between_range(first_ranges, last_ranges).map(Some)
    }

    fn ranges_at(&self, (layer, slot): Location) -> &[Range<u32>] {
        self.layers[layer].ranges(slot)
    }

    fn targets_at(&self, (layer, slot): Location) -> TargetLocations {
        self.layers[layer]
            .target_refs(slot)
            .map(|target| target.map(|(layer, id)| self.span_location_by_id(layer, id).unwrap()))
    }

    // Reverse DFS postorder visits each reachable dependent after both of its
    // affected parents. Keep the traversal iterative for deep target chains.
    fn dependent_order(&self, root: Location) -> (BTreeSet<Location>, Vec<Location>) {
        let mut visited = BTreeSet::new();
        let mut order = Vec::new();
        let mut stack = vec![(root, false)];
        let mut children = Vec::new();
        while let Some((location @ (layer, slot), expanded)) = stack.pop() {
            if expanded {
                if location != root {
                    order.push(location);
                }
            } else if visited.insert(location) {
                stack.push((location, true));
                self.extend_dependents(layer, slot, &mut children);
                stack.extend(children.drain(..).map(|child| (child, false)));
            }
        }
        order.reverse();
        (visited, order)
    }

    fn attach_target(&mut self, parent: Location, (layer, slot): Location) {
        Arc::make_mut(&mut self.layers[layer])
            .dependents
            .entry(parent)
            .or_default()
            .push(slot);
    }

    fn detach_target(&mut self, layer: usize, slot: usize) {
        if !self.layers[layer].has_implicit_target(slot) {
            let parents = self.targets_at((layer, slot));
            for parent in target_links(parents) {
                let dependents = &mut Arc::make_mut(&mut self.layers[layer]).dependents;
                let children = dependents.get_mut(&parent).unwrap();
                children.retain(|&child| child != slot);
                if children.is_empty() {
                    dependents.remove(&parent);
                }
            }
        }
    }

    fn extend_dependents(&self, layer: usize, slot: usize, pending: &mut Vec<(usize, usize)>) {
        let parent = &self.layers[layer];
        let id = parent.id_at(slot);
        for (index, candidate) in self.layers.iter().enumerate() {
            if let Some(children) = candidate.dependents.get(&(layer, slot)) {
                pending.extend(children.iter().map(|&child| (index, child)));
            }
            if candidate.implicit_target_layer() == Some(parent.id())
                && let Some(child) = candidate
                    .slot(id)
                    .filter(|&s| candidate.has_implicit_target(s))
            {
                pending.push((index, child));
            }
        }
    }
}

pub(crate) type Location = (usize, usize);
type TargetLocations = [Option<Location>; 2];
pub(crate) type TargetRefs<'a> = [Option<(&'a str, IdRef<'a>)>; 2];

fn target_links([first, last]: TargetLocations) -> impl Iterator<Item = Location> {
    first
        .into_iter()
        .chain(last.filter(|&last| Some(last) != first))
}

fn validate_endpoint_layers(first: &SpanRef, last: &SpanRef) -> Result<(), Error> {
    if first.layer() != last.layer() {
        return Err(Error::DifferentLayers);
    }
    Ok(())
}

fn between_range(first: &[Range<u32>], last: &[Range<u32>]) -> Result<Range<u32>, Error> {
    let (Some(start), Some(end)) = (first.first(), last.last()) else {
        return Err(Error::EmptyEndpoint);
    };
    if first.last().unwrap().end > last.first().unwrap().start {
        return Err(Error::Reversed);
    }
    Ok(start.start..end.end)
}

fn resolve_target_ranges<'a>(
    parents: TargetLocations,
    ranges: impl Fn(Location) -> &'a [Range<u32>],
) -> Result<Option<RangeSnapshot>, Error> {
    match parents {
        [Some(first), Some(last)] => Ok(Some(RangeSnapshot::One(between_range(
            ranges(first),
            ranges(last),
        )?))),
        [Some(parent), None] => Ok(Some(RangeSnapshot::new(ranges(parent)))),
        [None, _] => Ok(None),
    }
}
// Snapshot a target's ranges before mutating the layer that may own them.
enum RangeSnapshot {
    Empty,
    One(Range<u32>),
    Many(Vec<Range<u32>>),
}

impl RangeSnapshot {
    fn new(ranges: &[Range<u32>]) -> Self {
        match ranges {
            [] => Self::Empty,
            [range] => Self::One(range.clone()),
            ranges => Self::Many(ranges.to_vec()),
        }
    }

    fn as_slice(&self) -> &[Range<u32>] {
        match self {
            Self::Empty => &[],
            Self::One(range) => core::slice::from_ref(range),
            Self::Many(ranges) => ranges,
        }
    }
}
pub(crate) fn span_ranges_touch(ranges: &[Range<u32>], selection: &Selection) -> bool {
    ranges.iter().any(|span| {
        selection.ranges().iter().any(|hit| {
            if span.start == span.end {
                if hit.start == hit.end {
                    span.start == hit.start
                } else {
                    hit.start <= span.start && span.start <= hit.end
                }
            } else if hit.start == hit.end {
                false
            } else {
                span.start < hit.end && hit.start < span.end
            }
        })
    })
}

#[derive(Clone, Debug)]
pub(crate) struct MatchedSpan {
    pub(crate) layer_index: usize,
    pub(crate) span: Span,
    pub(crate) ranges: Vec<Range<u32>>,
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

/// An annotation targeting text ranges, another span, or a run of spans. It can carry an ID and attributes.
/// Unlike a [`Selection`], a span is an annotation that can be stored in a layer, not just a set of text positions.
#[derive(Clone, Debug)]
pub struct Span {
    pub(crate) id: Option<SpanId>,
    pub(crate) target: Target,
    pub(crate) attrs: Attributes,
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
    pub fn on(layer: impl Into<String>, id: impl Into<SpanId>) -> Self {
        Self {
            id: None,
            target: Target::Span(SpanRef::new(layer, id)),
            attrs: Attributes::default(),
        }
    }

    /// Creates a span targeting the referenced span.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Span, span_id};
    /// let verse = span_id("verses", "GEN 3:15");
    /// let note = Span::on_ref(&verse).with_attr("topic", "enmity"); // The new note points to the same verse.
    /// ```
    pub fn on_ref(reference: &SpanRef) -> Self {
        Self::on(reference.layer(), reference.id().clone())
    }

    /// Creates a span from the first endpoint's start through the last endpoint's end.
    ///
    /// Both endpoints must be in the same layer and have ranges. The first
    /// endpoint must end at or before the last one starts.
    ///
    /// ```
    /// use klause::{Span, span_id};
    /// let highlight = Span::between(span_id("words", 3_u64), span_id("words", 5_u64))
    ///     .with_attr("color", "yellow");
    /// ```
    pub fn between(first: SpanRef, last: SpanRef) -> Self {
        Self {
            id: None,
            target: Target::Between(first, last),
            attrs: Attributes::default(),
        }
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
    pub fn set_attr(mut self, key: impl Into<String>, values: impl IntoValues) -> Self {
        self.attrs.set(key.into(), values);
        self
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
    pub fn remove_attr(mut self, key: &str) -> Self {
        self.attrs.remove(key);
        self
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
    pub fn values(&self, key: &str) -> impl Iterator<Item = &Value> {
        self.attrs.values(key)
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
        Self {
            id: None,
            target: Target::Text(vec![range]),
            attrs: Attributes::default(),
        }
    }
}

/// Creates a span targeting a vector of document byte ranges.
impl From<Vec<Range<u32>>> for Span {
    fn from(mut ranges: Vec<Range<u32>>) -> Self {
        ranges.sort_by_key(|range| (range.start, range.end));
        let mut merged: Vec<Range<u32>> = Vec::with_capacity(ranges.len());
        for range in ranges {
            if let Some(last) = merged.last_mut()
                && last.start <= last.end
                && range.start <= range.end
                && range.start <= last.end
            {
                last.end = last.end.max(range.end);
                continue;
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

/// What a [`Span`] annotates: text ranges, another span, or a run between two spans.
#[derive(Clone, Debug)]
pub enum Target {
    Text(Vec<Range<u32>>),
    Span(SpanRef),
    /// A single range between two endpoints in the same layer, in text order.
    Between(SpanRef, SpanRef),
}

impl Target {
    pub(crate) fn target_refs(&self) -> TargetRefs<'_> {
        match self {
            Self::Text(_) => [None, None],
            Self::Span(reference) => [Some(reference.target_ref()), None],
            Self::Between(first, last) => [Some(first.target_ref()), Some(last.target_ref())],
        }
    }
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
    pub(crate) fn target_ref(&self) -> (&str, IdRef<'_>) {
        (self.layer(), self.id().into())
    }
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
    pub(crate) fn into_groups(self) -> Vec<(String, Vec<Value>)> {
        self.0
    }

    pub(crate) fn from_pairs<'a>(pairs: impl Iterator<Item = (&'a str, &'a Value)>) -> Self {
        let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
        for (key, value) in pairs {
            // The packed column preserves group order and keeps equal keys adjacent.
            if let Some((_, values)) = groups.last_mut().filter(|(last, _)| last == key) {
                values.push(value.clone());
            } else {
                groups.push((key.into(), vec![value.clone()]));
            }
        }
        Self(groups)
    }

    pub(crate) fn set(&mut self, key: String, values: impl IntoValues) {
        let values = values.into_values();
        if values.is_empty() {
            self.remove(&key);
        } else if let Some((_, stored)) = self.0.iter_mut().find(|(stored, _)| *stored == key) {
            *stored = values;
        } else {
            self.0.push((key, values));
        }
    }

    pub(crate) fn remove(&mut self, key: &str) {
        self.0.retain(|(stored, _)| stored != key);
    }
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
    /// The first Between endpoint ends after the last endpoint begins.
    Reversed,
    /// The endpoints of a Between target are in different layers.
    DifferentLayers,
    /// An endpoint of a Between target has no ranges.
    EmptyEndpoint,
    ReadOnlyLayer(String),
    OutOfRange(Range<u32>),
    NotCharBoundary(u32),
    SplitsCharacter,
    TooLong,
    NotOne {
        found: usize,
    },
    Source {
        layer: String,
        error: SourceError,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLayer(layer) => write!(f, "unknown layer: {layer}"),
            Self::DuplicateLayer(layer) => write!(f, "duplicate layer: {layer}"),
            Self::UnknownSpan(reference) => write!(f, "unknown span: {reference:?}"),
            Self::DuplicateId(reference) => write!(f, "duplicate span ID: {reference:?}"),
            Self::Cycle(reference) => {
                write!(f, "span replacement would create a cycle: {reference:?}")
            }
            Self::Reversed => f.write_str("Between endpoints are reversed or overlap"),
            Self::DifferentLayers => f.write_str("Between endpoints must be in the same layer"),
            Self::EmptyEndpoint => f.write_str("Between endpoints must have ranges"),
            Self::ReadOnlyLayer(layer) => write!(f, "layer is read-only: {layer}"),
            Self::OutOfRange(range) => write!(f, "range is outside the document: {range:?}"),
            Self::NotCharBoundary(position) => {
                write!(f, "position is not a UTF-8 character boundary: {position}")
            }
            Self::SplitsCharacter => f.write_str("push would change existing normalized text"),
            Self::TooLong => f.write_str("document text exceeds u32::MAX bytes"),
            Self::NotOne { found } => {
                write!(f, "expected exactly one range, found {found}")
            }
            Self::Source { layer, error } => {
                write!(f, "source layer {layer} failed: {error}")
            }
        }
    }
}

impl core::error::Error for Error {}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    fn fixture() -> (Document, SpanRef, SpanRef, SpanRef) {
        let mut doc = Document::new("edits")
            .with_layer("words")
            .with_layer("tags");
        doc.push("é abc xyz").unwrap();
        let first = doc.add_span("words", Span::from(0..2)).unwrap();
        let second = doc.add_span("words", Span::from(3..6)).unwrap();
        let tag = doc.add_span("tags", Span::on_ref(&first)).unwrap();
        (doc, first, second, tag)
    }

    #[test]
    fn retargeting_detaches_old_parent_and_updates_descendants() {
        let (mut doc, first, second, tag) = fixture();
        let child = doc.add_span("tags", Span::on_ref(&tag)).unwrap();
        doc.replace_span(&tag, Span::on_ref(&second).with_id("ignored"))
            .unwrap();
        doc.remove_span(&first).unwrap();
        assert_eq!(doc.span(&tag).unwrap().id(), Some(tag.id()));
        assert_eq!(doc.ranges(&child).unwrap(), [3..6]);
        doc.replace_span(&second, Span::from([0..2, 7..10]))
            .unwrap();
        assert_eq!(doc.ranges(&child).unwrap(), [0..2, 7..10]);
        doc.remove_span(&second).unwrap();
        assert!(doc.span(&tag).is_none());
        assert!(doc.ranges(&child).is_none());
    }

    #[test]
    fn replacement_validation_is_atomic() {
        let (mut doc, first, _, tag) = fixture();
        for invalid in [
            Span::from(0..1),
            Span::from(0..11),
            Span::from(Range { start: 6, end: 3 }),
        ] {
            assert!(doc.replace_span(&first, invalid).is_err());
            assert_eq!(doc.ranges(&first).unwrap(), [0..2]);
            assert_eq!(doc.ranges(&tag).unwrap(), [0..2]);
        }
        assert!(matches!(
            doc.replace_span(&first, Span::on_ref(&tag)),
            Err(Error::Cycle(_))
        ));
        doc.remove_span(&first).unwrap();
        assert!(doc.span(&tag).is_none());
    }

    #[test]
    fn failed_add_and_push_do_not_leave_reverse_edges_or_consume_ids() {
        let (mut doc, first, _, tag) = fixture();
        assert!(matches!(
            doc.add_span("tags", Span::on_ref(&first).with_id(tag.id().clone())),
            Err(Error::DuplicateId(_))
        ));
        let before = String::from(doc.text());
        assert!(
            doc.push_span("tags", "!", |span| span.with_id(tag.id().clone()))
                .is_err()
        );
        assert_eq!(doc.text(), before);
        let next = doc.add_span("tags", Span::on_ref(&first)).unwrap();
        assert_eq!(next.id(), &SpanId::Int(2));
        doc.remove_span(&first).unwrap();
        assert_eq!(doc.layer("tags").unwrap().spans().count(), 0);
    }

    #[test]
    fn reused_slots_do_not_retain_dependencies_or_ranges() {
        let (mut doc, first, second, tag) = fixture();
        doc.remove_span(&tag).unwrap();
        let replacement = doc.add_span("tags", Span::on_ref(&second)).unwrap();
        doc.remove_span(&first).unwrap();
        assert_eq!(doc.ranges(&replacement).unwrap(), [3..6]);
        for _ in 0..100 {
            let child = doc.add_span("tags", Span::on_ref(&replacement)).unwrap();
            assert_eq!(doc.span_location(&child), Some((1, 1)));
            doc.remove_span(&child).unwrap();
        }
        assert_eq!(doc.span_location(&replacement), Some((1, 0)));
        doc.remove_span(&second).unwrap();
        assert!(doc.span(&replacement).is_none());
        assert_eq!(doc.layer("tags").unwrap().spans().count(), 0);
    }

    #[test]
    fn removing_largest_integer_id_restores_assignment_after_remaining_maximum() {
        let mut doc = Document::new("ids").with_layer("spans");
        doc.add_span("spans", Span::from(0..0).with_id("string"))
            .unwrap();
        doc.add_span("spans", Span::from(0..0).with_id(2_u64))
            .unwrap();
        let max = doc
            .add_span("spans", Span::from(0..0).with_id(u64::MAX))
            .unwrap();
        doc.remove_span(&max).unwrap();
        let next = doc.add_span("spans", Span::from(0..0)).unwrap();
        assert_eq!(next.id(), &SpanId::Int(3));
    }

    #[test]
    fn reverse_dependencies_are_allocated_only_for_targeted_slots() {
        let mut doc = Document::new("lazy").with_layer("spans");
        let first = doc.add_span("spans", Span::from(0..0)).unwrap();
        let second = doc.add_span("spans", Span::from(0..0)).unwrap();
        assert!(doc.layer("spans").unwrap().dependents.is_empty());
        doc.add_span("spans", Span::on_ref(&second)).unwrap();
        assert_eq!(doc.layer("spans").unwrap().dependents.len(), 1);
        doc.remove_span(&first).unwrap();
        doc.replace_span(&second, Span::from(Vec::<Range<u32>>::new()))
            .unwrap();
        doc.remove_span(&second).unwrap();
        assert_eq!(doc.layer("spans").unwrap().spans().count(), 0);
    }

    #[test]
    fn deep_chains_update_and_remove_without_recursion() {
        let mut doc = Document::new("deep").with_layer("spans");
        doc.push("abc").unwrap();
        let root = doc.add_span("spans", Span::from(0..1)).unwrap();
        let mut leaf = root.clone();
        for _ in 0..20_000 {
            leaf = doc.add_span("spans", Span::on_ref(&leaf)).unwrap();
        }
        doc.replace_span(&root, Span::from(2..3)).unwrap();
        assert_eq!(doc.ranges(&leaf).unwrap(), [2..3]);
        doc.remove_span(&root).unwrap();
        assert!(doc.span(&leaf).is_none());
        assert_eq!(doc.layer("spans").unwrap().spans().count(), 0);
    }

    #[test]
    fn attributes_preserve_key_order_and_empty_replacements_remove_keys() {
        let doc = Document::new("attrs")
            .with_attr("a", [1, 2])
            .with_attr("b", 3)
            .set_attr("a", [4, 5]);
        assert_eq!(doc.attr("a"), Some(&Value::Int(4)));
        assert_eq!(doc.values("a").count(), 2);
        assert_eq!(
            doc.attrs().map(|(key, _)| key).collect::<Vec<_>>(),
            ["a", "a", "b"]
        );
        assert!(doc.remove_attr("a").attr("a").is_none());
        let span = Span::from(0..0)
            .with_attr("a", 1)
            .set_attr("a", Vec::<Value>::new());
        assert_eq!(span.attrs().count(), 0);
    }

    #[test]
    fn attribute_index_tracks_added_replaced_removed_and_reused_spans() {
        let mut doc = Document::new("filter-index").with_layer("morph");
        doc.push("abc").unwrap();
        doc.add_span("morph", Span::from(0..1).with_attr("pos", "noun"))
            .unwrap();
        let verbs = || crate::selection::spans_in("morph").with_attr("pos", "verb");

        assert!(doc.get_selection(verbs()).unwrap().is_empty()); // Build the empty-result index.

        let verb = doc
            .add_span("morph", Span::from(1..2).with_attr("pos", "verb"))
            .unwrap();
        assert_eq!(doc.get_selection(verbs()).unwrap().ranges(), [1..2]);
        assert_eq!(doc.find().count_spans(verbs()).unwrap(), 1);

        doc.replace_span(&verb, Span::from(1..2).with_attr("pos", "noun"))
            .unwrap();
        assert!(doc.get_selection(verbs()).unwrap().is_empty());

        doc.remove_span(&verb).unwrap();
        assert_eq!(
            doc.add_span("morph", Span::from(2..3).with_attr("pos", "verb"))
                .unwrap()
                .id(),
            &SpanId::Int(2)
        );
        assert_eq!(doc.get_selection(verbs()).unwrap().ranges(), [2..3]);
    }

    #[test]
    fn borrowed_reads_survive_other_reads_and_refresh_on_reuse() {
        let mut doc = Document::new("borrowed").with_layer("words");
        doc.push("abcdef").unwrap();
        let late = doc
            .add_span("words", Span::from(1..2).with_id(90_u64))
            .unwrap();
        let named = doc
            .add_span("words", Span::from(1..2).with_id("named"))
            .unwrap();
        let early = doc
            .add_span("words", Span::from(1..2).with_id(5_u64))
            .unwrap();
        let borrowed = doc.span(&late).unwrap();
        let ids: Vec<_> = doc
            .layer("words")
            .unwrap()
            .spans()
            .map(|span| span.id().unwrap())
            .collect();
        assert_eq!(ids, [early.id(), late.id(), named.id()]);
        assert!(core::ptr::eq(borrowed, doc.span(&late).unwrap()));

        doc.replace_span(
            &late,
            Span::from([0..1, 3..4]).with_attr("form", "replacement"),
        )
        .unwrap();
        let borrowed = doc.span(&late).unwrap();
        assert_eq!(borrowed.attr("form"), Some(&Value::from("replacement")));
        assert!(matches!(borrowed.target(), Target::Text(ranges) if ranges == &[0..1, 3..4]));

        doc.remove_span(&late).unwrap();
        let added = doc.add_span("words", Span::from(5..6)).unwrap();
        assert_eq!(doc.span_location(&added), Some((0, 0)));
        assert_eq!(doc.span(&added).unwrap().attrs().count(), 0);
        assert_eq!(doc.ranges(&added).unwrap(), [5..6]);
    }

    #[test]
    fn interning_preserves_attribute_order_and_exact_float_representations() {
        let mut doc = Document::new("attributes").with_layer("spans");
        doc.push("ab").unwrap();
        let nan = f64::from_bits(0x7ff8_0000_0000_0001);
        let tagged = doc
            .add_span(
                "spans",
                Span::from(0..1)
                    .with_attr("z", [Value::Int(0), Value::Float(-0.0), Value::Float(nan)])
                    .with_attr("a", ["dup", "dup"]),
            )
            .unwrap();
        doc.add_span("spans", Span::from(1..2).with_attr("z", 0.0))
            .unwrap();

        let span = doc.span(&tagged).unwrap();
        let attrs: Vec<_> = span.attrs().collect();
        assert_eq!(
            attrs.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            ["z", "z", "z", "a", "a"]
        );
        assert!(
            matches!(attrs[1].1, Value::Float(value) if value.to_bits() == (-0.0_f64).to_bits())
        );
        assert!(matches!(attrs[2].1, Value::Float(value) if value.to_bits() == nan.to_bits()));
        let with_value = |value| crate::spans_in("spans").with_attr("z", value);
        assert_eq!(
            doc.find()
                .count_spans(with_value(Value::Float(-0.0)))
                .unwrap(),
            2
        );
        assert_eq!(
            doc.find().count_spans(with_value(Value::Int(0))).unwrap(),
            2
        );
        assert_eq!(
            doc.find()
                .count_spans(with_value(Value::Float(nan)))
                .unwrap(),
            0
        );
        assert_eq!(
            doc.find()
                .count_spans(crate::spans_in("spans").with_attr("a", "dup"))
                .unwrap(),
            1
        );
    }

    #[test]
    fn inferred_and_explicit_edges_survive_mixed_target_edits() {
        let (mut doc, first, second, tag) = fixture();
        let tag_two = doc.add_span("tags", Span::on_ref(&second)).unwrap();
        let via_tag = doc.add_span("tags", Span::on_ref(&tag)).unwrap();
        assert!(doc.layer("words").unwrap().dependents.is_empty());
        assert_eq!(doc.layer("tags").unwrap().dependents.len(), 1);

        doc.replace_span(&tag, Span::from(2..2)).unwrap();
        doc.remove_span(&first).unwrap();
        assert_eq!(doc.ranges(&via_tag).unwrap(), [2..2]);
        assert_eq!(doc.ranges(&tag_two).unwrap(), [3..6]);
        doc.replace_span(&tag, Span::on_ref(&second)).unwrap();
        doc.replace_span(&second, Span::from([0..2, 7..10]))
            .unwrap();
        assert_eq!(doc.ranges(&via_tag).unwrap(), [0..2, 7..10]);
        assert_eq!(doc.ranges(&tag_two).unwrap(), [0..2, 7..10]);
        doc.remove_span(&second).unwrap();
        assert_eq!(doc.layer("tags").unwrap().spans().count(), 0);
    }

    #[test]
    fn snapshots_share_frozen_layers_across_overlay_edits() {
        fn assert_snapshot<T: Send + Sync + Clone>() {}
        assert_snapshot::<Document>();

        let (doc, first, second, tag) = fixture();
        let base = doc.freeze_layers();
        let mut doc = base.clone().with_layer("highlights");
        let highlight = doc
            .add_span("highlights", Span::between(first.clone(), second.clone()))
            .unwrap();
        let alias = doc.add_span("highlights", Span::on_ref(&tag)).unwrap();
        let snapshot = doc.clone();
        doc.replace_span(&highlight, Span::from(7..10)).unwrap();
        doc.remove_span(&alias).unwrap();

        for i in 0..base.layers.len() {
            assert!(Arc::ptr_eq(&base.layers[i], &doc.layers[i]));
            assert!(Arc::ptr_eq(&snapshot.layers[i], &doc.layers[i]));
        }
        assert!(!Arc::ptr_eq(&snapshot.layers[2], &doc.layers[2]));
        assert!(base.layer("highlights").is_none());
        assert_eq!(snapshot.ranges(&highlight).unwrap(), [0..6]);
        assert_eq!(doc.ranges(&highlight).unwrap(), [7..10]);
        assert!(snapshot.span(&alias).is_some());
        assert!(doc.span(&alias).is_none());
    }

    #[test]
    fn frozen_layers_reject_edits_and_dependent_cascades_atomically() {
        let (mut doc, first, _, tag) = fixture();
        let alias = doc.add_span("words", Span::on_ref(&tag)).unwrap();
        doc.add_layer("frozen").unwrap();
        let frozen = doc.add_span("frozen", Span::on_ref(&alias)).unwrap();
        doc.freeze_layer("frozen").unwrap();
        assert!(doc.layer("frozen").unwrap().is_frozen());
        assert!(!doc.layer("words").unwrap().is_frozen());
        for error in [
            doc.add_span("frozen", Span::from(2..2)).unwrap_err(),
            doc.replace_span(&frozen, Span::from(2..2)).unwrap_err(),
            doc.remove_span(&frozen).unwrap_err(),
            doc.push_span("frozen", "unused", |span| span).unwrap_err(),
            doc.replace_span(&first, Span::from(2..2)).unwrap_err(),
            doc.remove_span(&first).unwrap_err(),
        ] {
            assert!(matches!(error, Error::ReadOnlyLayer(id) if id == "frozen"));
        }
        assert_eq!(doc.text(), "é abc xyz");
        for span in [&first, &tag, &alias, &frozen] {
            assert_eq!(doc.ranges(span).unwrap(), [0..2]);
        }
        doc.replace_span(&first, Span::from(0..2).with_attr("status", "ok"))
            .unwrap();
        assert_eq!(doc.ranges(&frozen).unwrap(), [0..2]);
    }

    #[test]
    fn removing_an_editable_endpoint_leaves_the_snapshot_unchanged() {
        let (mut doc, first, second, tag) = fixture();
        let overlay = doc
            .add_span("tags", Span::between(first.clone(), second.clone()))
            .unwrap();
        let snapshot = doc.clone();
        doc.remove_span(&first).unwrap();
        assert!(doc.span(&tag).is_none());
        assert!(doc.span(&overlay).is_none());
        assert!(doc.span(&second).is_some());
        assert_eq!(snapshot.ranges(&overlay).unwrap(), [0..6]);
        assert_eq!(snapshot.ranges(&tag).unwrap(), [0..2]);
        assert!(snapshot.span(&first).is_some());
    }
}
