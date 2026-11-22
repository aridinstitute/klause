use alloc::{boxed::Box, string::String, sync::Arc, vec, vec::Vec};
use core::{
    fmt,
    ops::{Range, RangeFrom, RangeFull},
};

use crate::document::{Document, Error, Span, SpanId, SpanRef, Target, Value};
use crate::layer::{IdRef, Layer};

/// A set of text ranges in a document, represented as sorted, non-overlapping byte ranges.
/// Unlike a [`crate::Span`], a selection describes text positions and has no annotation attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    ranges: Vec<Range<u32>>,
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
        &self.ranges
    }

    pub(crate) fn into_ranges(self) -> Vec<Range<u32>> {
        self.ranges
    }

    pub(crate) fn retain_ranges(&mut self, keep: impl FnMut(&Range<u32>) -> bool) {
        self.ranges.retain(keep);
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
        let mut ranges = self.ranges.clone();
        ranges.extend(_other.ranges.iter().cloned());
        Self::from_ranges(ranges)
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
        let mut ranges = Vec::new();

        for left in self.ranges.iter().filter(|range| range.start < range.end) {
            for right in _other.ranges.iter().filter(|range| range.start < range.end) {
                let start = left.start.max(right.start);
                let end = left.end.min(right.end);
                if start < end {
                    ranges.push(start..end);
                }
            }
        }

        for point in self.ranges.iter().filter(|range| range.start == range.end) {
            if _other
                .ranges
                .iter()
                .any(|range| contains_point(range, point.start))
            {
                ranges.push(point.clone());
            }
        }
        for point in _other
            .ranges
            .iter()
            .filter(|range| range.start == range.end)
        {
            if self
                .ranges
                .iter()
                .any(|range| contains_point(range, point.start))
            {
                ranges.push(point.clone());
            }
        }

        Self::from_ranges(ranges)
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
    pub fn subtract(&self, other: &Selection) -> Selection {
        let mut ranges = Vec::new();
        let mut cuts = other.ranges.iter().peekable();
        for left in &self.ranges {
            while cuts.peek().is_some_and(|cut| cut.end < left.start) {
                cuts.next();
            }

            if left.start == left.end {
                // A point is removed even at the end of a cut. A point cut,
                // on the other hand, never removes bytes from a nonempty range.
                if !cuts
                    .peek()
                    .is_some_and(|cut| contains_point(cut, left.start))
                {
                    ranges.push(left.clone());
                }
                continue;
            }

            let mut start = left.start;
            while let Some(cut) = cuts.peek().filter(|cut| cut.start < left.end) {
                if cut.start == cut.end || cut.end <= start {
                    cuts.next();
                    continue;
                }
                if start < cut.start {
                    ranges.push(start..cut.start);
                }
                start = cut.end;
                if start >= left.end {
                    // The same cut might also cover a later range or point.
                    break;
                }
                cuts.next();
            }
            if start < left.end {
                ranges.push(start..left.end);
            }
        }
        // Subtraction only shrinks or splits normalized ranges, so the output
        // is already sorted and disjoint, with any surviving points isolated.
        Self { ranges }
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
        self.ranges.iter().any(|range| {
            if range.start == range.end {
                range.start == _position
            } else {
                range.start <= _position && _position < range.end
            }
        })
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
        self.ranges.is_empty()
    }

    pub(crate) fn from_ranges(ranges: Vec<Range<u32>>) -> Self {
        // The common case is already normalized (e.g. one range per word).
        if ranges.iter().all(|range| range.start <= range.end)
            && ranges.windows(2).all(|pair| pair[0].end < pair[1].start)
        {
            return Self { ranges };
        }
        let mut non_empty = Vec::new();
        let mut points = Vec::new();
        for range in ranges {
            if range.start < range.end {
                non_empty.push(range);
            } else if range.start == range.end {
                points.push(range.start);
            }
        }
        non_empty.sort_by_key(|range| (range.start, range.end));

        let mut merged: Vec<Range<u32>> = Vec::with_capacity(non_empty.len());
        for range in non_empty {
            if let Some(last) = merged.last_mut()
                && range.start <= last.end
            {
                last.end = last.end.max(range.end);
            } else {
                merged.push(range);
            }
        }

        points.sort_unstable();
        points.dedup();
        let mut ranges = merged.clone();
        for point in points {
            if !merged
                .iter()
                .any(|range| range.start <= point && point <= range.end)
            {
                ranges.push(point..point);
            }
        }
        ranges.sort_by_key(|range| (range.start, range.end));
        Self { ranges }
    }
}

fn contains_point(range: &Range<u32>, point: u32) -> bool {
    if range.start == range.end {
        range.start == point
    } else {
        range.start <= point && point <= range.end
    }
}

/// Text described against a document, resolved when a query step or passage runs.
pub trait IntoSelection {
    /// Whether this selector might contain point ranges. Unknown selectors are
    /// conservative so planning preserves their order among selection steps.
    #[doc(hidden)]
    fn may_select_points(&self, _doc: &Document) -> bool {
        true
    }

    /// Estimates candidate work for query planning without resolving the selector.
    ///
    /// Custom selectors may keep the default conservative estimate. This hint
    /// affects execution order, never whether a selector is evaluated.
    #[doc(hidden)]
    fn estimate_cost(&self, doc: &Document) -> usize {
        doc.len() as usize
    }

    /// Resolves this selector and intersects it with already narrowed query hits.
    #[doc(hidden)]
    fn into_selection_within(self, doc: &Document, hits: &Selection) -> Result<Selection, Error>
    where
        Self: Sized,
    {
        Ok(hits.intersect(&self.into_selection(doc)?))
    }

    /// Resolves this description into text ranges for the given document.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, IntoSelection, Span, span_id};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// let reference = span_id("verses", "GEN 3:15");
    /// let selection = reference.into_selection(&doc)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn into_selection(self, doc: &Document) -> Result<Selection, Error>;
}

impl IntoSelection for Range<u32> {
    fn may_select_points(&self, _doc: &Document) -> bool {
        self.is_empty()
    }

    fn estimate_cost(&self, _doc: &Document) -> usize {
        1
    }

    fn into_selection(self, doc: &Document) -> Result<Selection, Error> {
        doc.selection_from_ranges(vec![self])
    }
}

impl IntoSelection for RangeFull {
    fn may_select_points(&self, doc: &Document) -> bool {
        doc.is_empty()
    }

    fn estimate_cost(&self, _doc: &Document) -> usize {
        1
    }

    fn into_selection(self, doc: &Document) -> Result<Selection, Error> {
        doc.selection_from_ranges(core::iter::once(0..doc.len()).collect())
    }
}

impl IntoSelection for Vec<Range<u32>> {
    fn may_select_points(&self, _doc: &Document) -> bool {
        self.iter().any(Range::is_empty)
    }

    fn estimate_cost(&self, _doc: &Document) -> usize {
        self.len()
    }

    fn into_selection(self, doc: &Document) -> Result<Selection, Error> {
        doc.selection_from_ranges(self)
    }
}

impl IntoSelection for Selection {
    fn may_select_points(&self, _doc: &Document) -> bool {
        self.ranges.iter().any(Range::is_empty)
    }

    fn estimate_cost(&self, _doc: &Document) -> usize {
        self.ranges.len()
    }

    fn into_selection(self, doc: &Document) -> Result<Selection, Error> {
        doc.selection_from_ranges(self.ranges)
    }
}

impl IntoSelection for &Selection {
    fn may_select_points(&self, _doc: &Document) -> bool {
        self.ranges.iter().any(Range::is_empty)
    }

    fn estimate_cost(&self, _doc: &Document) -> usize {
        self.ranges.len()
    }

    fn into_selection(self, doc: &Document) -> Result<Selection, Error> {
        doc.selection_from_ranges(self.ranges.clone())
    }
}

impl IntoSelection for SpanRef {
    fn may_select_points(&self, doc: &Document) -> bool {
        doc.ranges(self)
            .is_none_or(|ranges| ranges.iter().any(Range::is_empty))
    }

    fn estimate_cost(&self, doc: &Document) -> usize {
        doc.ranges(self).map_or(1, <[_]>::len)
    }

    fn into_selection(self, doc: &Document) -> Result<Selection, Error> {
        let ranges = doc
            .ranges(&self)
            .ok_or_else(|| Error::UnknownSpan(self.clone()))?;
        doc.selection_from_ranges(ranges.to_vec())
    }
}

impl IntoSelection for SpanFilter {
    fn may_select_points(&self, doc: &Document) -> bool {
        doc.filter_may_select_points(self)
    }

    fn estimate_cost(&self, doc: &Document) -> usize {
        doc.estimate_filter_cost(self)
    }

    fn into_selection_within(self, doc: &Document, hits: &Selection) -> Result<Selection, Error> {
        doc.selection_for_filter_within(&self, hits)
    }

    fn into_selection(self, doc: &Document) -> Result<Selection, Error> {
        doc.selection_for_filter(&self)
    }
}

impl<const N: usize> IntoSelection for [Range<u32>; N] {
    fn may_select_points(&self, _doc: &Document) -> bool {
        self.iter().any(Range::is_empty)
    }

    fn estimate_cost(&self, _doc: &Document) -> usize {
        N
    }

    fn into_selection(self, doc: &Document) -> Result<Selection, Error> {
        doc.selection_from_ranges(Vec::from(self))
    }
}

// A query can inspect either source spans or packed stored spans without
// materializing the latter, even when a filter follows a target chain.
#[derive(Clone, Copy)]
pub(crate) enum SpanView<'a> {
    Borrowed(&'a Span),
    Stored(&'a Layer, usize),
}

impl<'a> SpanView<'a> {
    fn id(self) -> Option<IdRef<'a>> {
        match self {
            Self::Borrowed(span) => span.id().map(Into::into),
            Self::Stored(layer, slot) => Some(layer.id_at(slot)),
        }
    }

    fn target(self) -> Option<(&'a str, IdRef<'a>)> {
        match self {
            Self::Borrowed(span) => match span.target() {
                Target::Span(reference) => Some((reference.layer(), reference.id().into())),
                Target::Text(_) => None,
            },
            Self::Stored(layer, slot) => layer.target_at(slot),
        }
    }

    pub(crate) fn values(self, key: &str) -> impl Iterator<Item = &'a Value> {
        let (borrowed, stored) = match self {
            Self::Borrowed(span) => (Some(span), None),
            Self::Stored(layer, slot) => {
                (None, layer.attr_key(key).map(|code| (layer, slot, code)))
            }
        };
        borrowed
            .into_iter()
            .flat_map(move |span| span.values(key))
            .chain(
                stored
                    .into_iter()
                    .flat_map(|(layer, slot, code)| layer.values(slot, code)),
            )
    }
}
/// Criteria for matching spans, such as spans in `morph` whose `pos` is `verb`.
#[derive(Clone)]
pub struct SpanFilter {
    layers: Option<Vec<String>>,
    conditions: Vec<Condition>,
}

#[derive(Clone)]
enum Condition {
    WithAttr(
        String,
        Arc<dyn Fn(&Value) -> bool + Send + Sync + 'static>,
        Option<Value>,
    ),
    WithoutAttr(String, Value),
    WithKey(String),
    WithoutKey(String),
    WithId(SpanId),
    Targeting(Box<SpanFilter>),
}

impl fmt::Debug for SpanFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpanFilter")
            .field("layers", &self.layers)
            .field("conditions", &self.conditions.len())
            .finish()
    }
}

impl SpanFilter {
    fn in_layers(layers: Option<Vec<String>>) -> Self {
        Self {
            layers,
            conditions: Vec::new(),
        }
    }

    pub(crate) fn layer_ids(&self) -> Option<&[String]> {
        self.layers.as_deref()
    }

    pub(crate) fn has_conditions(&self) -> bool {
        !self.conditions.is_empty()
    }

    pub(crate) fn index_hint(&self) -> Option<(&str, &Value)> {
        self.conditions
            .iter()
            .find_map(|condition| match condition {
                Condition::WithAttr(key, _, Some(value)) => Some((key.as_str(), value)),
                _ => None,
            })
    }

    pub(crate) fn matches(&self, doc: &Document, span: SpanView<'_>, layer_id: &str) -> bool {
        if self
            .layers
            .as_ref()
            .is_some_and(|layers| !layers.iter().any(|layer| layer == layer_id))
        {
            return false;
        }
        self.conditions.iter().all(|condition| match condition {
            Condition::WithAttr(key, matcher, _) => span.values(key).any(|value| matcher(value)),
            Condition::WithoutAttr(key, excluded) => {
                !span.values(key).any(|value| value == excluded)
            }
            Condition::WithKey(key) => span.values(key).next().is_some(),
            Condition::WithoutKey(key) => span.values(key).next().is_none(),
            Condition::WithId(id) => span.id() == Some(id.into()),
            Condition::Targeting(target_filter) => span.target().is_some_and(|(layer, id)| {
                doc.span_view(layer, id)
                    .is_some_and(|target| target_filter.matches(doc, target, layer))
            }),
        })
    }
}

impl SpanFilter {
    /// Requires spans to have an attribute matching the supplied matcher.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, spans_in};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let nouns = spans_in("words").with_attr("pos", "noun");
    /// let verbs = spans_in("words").with_attr("pos", "verb");
    /// assert_eq!(doc.get_selection(nouns)?.ranges(), &[15..21]); // “enmity”
    /// assert!(doc.get_selection(verbs)?.is_empty()); // No verb is tagged.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_attr<M: ValueMatcher + 'static>(
        mut self,
        key: impl Into<String>,
        matcher: M,
    ) -> Self {
        let index_hint = matcher.index_value();
        let matcher = Arc::new(matcher);
        self.conditions.push(Condition::WithAttr(
            key.into(),
            Arc::new(move |value| matcher.matches(value)),
            index_hint,
        ));
        self
    }

    /// Excludes spans having the specified attribute value.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, spans_in};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let non_verbs = spans_in("words").without_attr("pos", "verb");
    /// let hits = doc.get_selection(non_verbs)?; // “enmity” is still selected: it is tagged as a noun.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn without_attr(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.conditions
            .push(Condition::WithoutAttr(key.into(), value.into()));
        self
    }

    /// Requires spans to contain the specified attribute key.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, spans_in};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let tagged_words = spans_in("words").with_key("pos");
    /// let hits = doc.get_selection(tagged_words)?; // Only the tagged word “enmity” is selected.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.conditions.push(Condition::WithKey(key.into()));
        self
    }

    /// Excludes spans containing the specified attribute key.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, spans_in};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let untagged_words = spans_in("words").without_key("pos");
    /// let hits = doc.get_selection(untagged_words)?; // The untagged word “put” is selected.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn without_key(mut self, key: impl Into<String>) -> Self {
        self.conditions.push(Condition::WithoutKey(key.into()));
        self
    }

    /// Restricts matching to spans with the specified identifier.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, spans_in};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// let enmity = spans_in("words").with_id("enmity");
    /// let hits = doc.get_selection(enmity)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_id(mut self, id: impl Into<SpanId>) -> Self {
        self.conditions.push(Condition::WithId(id.into()));
        self
    }

    /// Restricts matching to spans whose target matches another filter.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{Document, Span, spans_in};
    /// # let mut doc = Document::new("GEN 3:15");
    /// # let range = doc.push("And I will put enmity")?;
    /// # doc.add_layer("verses")?;
    /// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
    /// # doc.add_layer("words")?;
    /// # doc.add_span("words", Span::from(11..14).with_id("put"))?;
    /// # doc.add_span("words", Span::from(15..21).with_id("enmity").with_attr("pos", "noun"))?;
    /// # doc.add_layer("notes")?;
    /// # doc.add_span("notes", Span::on_ref(&verse).with_attr("topic", "enmity"))?;
    /// let verse = spans_in("verses").with_id("GEN 3:15");
    /// let notes = spans_in("notes").targeting(verse);
    /// let hits = doc.get_selection(notes)?; // The note on Genesis 3:15 is selected.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn targeting(mut self, target: SpanFilter) -> Self {
        self.conditions.push(Condition::Targeting(Box::new(target)));
        self
    }
}

impl From<&str> for SpanFilter {
    /// Creates a filter for spans in the named layer.
    fn from(layer: &str) -> Self {
        spans_in(layer)
    }
}

/// A condition on an attribute value, such as equality, a set of values, or a numeric range.
pub trait ValueMatcher: Send + Sync {
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

    /// An exact value that can be used to narrow indexed candidates.
    fn index_value(&self) -> Option<Value> {
        None
    }
}

impl ValueMatcher for Value {
    fn matches(&self, value: &Value) -> bool {
        self == value
    }

    fn index_value(&self) -> Option<Value> {
        Some(self.clone())
    }
}

impl ValueMatcher for &str {
    fn matches(&self, value: &Value) -> bool {
        matches!(value, Value::Str(stored) if stored == *self)
    }

    fn index_value(&self) -> Option<Value> {
        Some(Value::from(*self))
    }
}

impl ValueMatcher for String {
    fn matches(&self, value: &Value) -> bool {
        matches!(value, Value::Str(stored) if stored == self)
    }

    fn index_value(&self) -> Option<Value> {
        Some(Value::from(self.clone()))
    }
}

impl ValueMatcher for i32 {
    fn matches(&self, value: &Value) -> bool {
        Value::Int(i64::from(*self)) == *value
    }

    fn index_value(&self) -> Option<Value> {
        Some(Value::Int(i64::from(*self)))
    }
}

impl ValueMatcher for i64 {
    fn matches(&self, value: &Value) -> bool {
        Value::Int(*self) == *value
    }

    fn index_value(&self) -> Option<Value> {
        Some(Value::Int(*self))
    }
}

impl ValueMatcher for f64 {
    fn matches(&self, value: &Value) -> bool {
        Value::Float(*self) == *value
    }

    fn index_value(&self) -> Option<Value> {
        Some(Value::Float(*self))
    }
}

impl ValueMatcher for bool {
    fn matches(&self, value: &Value) -> bool {
        matches!(value, Value::Bool(stored) if stored == self)
    }

    fn index_value(&self) -> Option<Value> {
        Some(Value::Bool(*self))
    }
}

macro_rules! range_matcher {
    ($range:ty) => {
        impl ValueMatcher for $range {
            fn matches(&self, value: &Value) -> bool {
                let Some(number) = numeric_value(value) else {
                    return false;
                };
                number >= self.start as f64 && number < self.end as f64
            }
        }
    };
}

range_matcher!(Range<i32>);
range_matcher!(Range<i64>);
range_matcher!(Range<f64>);

macro_rules! range_from_matcher {
    ($range:ty) => {
        impl ValueMatcher for $range {
            fn matches(&self, value: &Value) -> bool {
                let Some(number) = numeric_value(value) else {
                    return false;
                };
                number >= self.start as f64
            }
        }
    };
}

range_from_matcher!(RangeFrom<i32>);
range_from_matcher!(RangeFrom<i64>);
range_from_matcher!(RangeFrom<f64>);

fn numeric_value(value: &Value) -> Option<f64> {
    match value {
        Value::Int(value) => Some(*value as f64),
        Value::Float(value) => Some(*value),
        _ => None,
    }
}

/// A value matcher that accepts any of the values supplied to [`any_of`].
#[derive(Clone, Debug)]
pub struct AnyOf {
    values: Vec<Value>,
}

impl ValueMatcher for AnyOf {
    fn matches(&self, value: &Value) -> bool {
        self.values.iter().any(|candidate| candidate == value)
    }
}

/// Creates a matcher that accepts any of the supplied values.
///
/// # Example
///
/// ```no_run
/// # use klause::{any_of, spans_in};
/// let words = spans_in("words").with_attr("pos", any_of(["noun", "verb"]));
/// ```
pub fn any_of<T: Into<Value>>(values: impl IntoIterator<Item = T>) -> AnyOf {
    AnyOf {
        values: values.into_iter().map(Into::into).collect(),
    }
}

/// Creates a reference to one span by layer and identifier.
///
/// # Example
///
/// ```no_run
/// # use klause::{Document, Span, span_id};
/// # let mut doc = Document::new("GEN 3:15");
/// # let range = doc.push("And I will put enmity")?;
/// # doc.add_layer("verses")?;
/// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
/// let verse = span_id("verses", "GEN 3:15");
/// let passage = doc.get_passage(verse)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn span_id(layer: impl Into<String>, id: impl Into<SpanId>) -> SpanRef {
    SpanRef::new(layer, id)
}

/// Creates a filter matching spans in the named layer.
///
/// # Example
///
/// ```no_run
/// # use klause::{Document, Span, spans_in};
/// # let mut doc = Document::new("GEN 3:15");
/// # let range = doc.push("And I will put enmity")?;
/// # doc.add_layer("verses")?;
/// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
/// let verses = spans_in("verses");
/// let hits = doc.get_selection(verses)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn spans_in(layer: impl Into<String>) -> SpanFilter {
    SpanFilter::in_layers(Some(vec![layer.into()]))
}

/// Creates a filter matching spans in every layer.
///
/// # Example
///
/// ```no_run
/// # use klause::{Document, Span, all_spans};
/// # let mut doc = Document::new("GEN 3:15");
/// # let range = doc.push("And I will put enmity")?;
/// # doc.add_layer("verses")?;
/// # let verse = doc.add_span("verses", Span::from(range.clone()).with_id("GEN 3:15"))?;
/// let hits = doc.get_selection(all_spans())?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn all_spans() -> SpanFilter {
    SpanFilter::in_layers(None)
}
