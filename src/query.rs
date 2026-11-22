use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use core::{cmp::Ordering, ops::Range};
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::document::{Document, Error, MatchedSpan, Span, SpanId, Value, span_ranges_touch};
use crate::passage::Passage;
use crate::selection::{IntoSelection, Selection, SpanFilter, span_id, spans_in};

trait PlannedSelection {
    fn estimate_cost(&self, doc: &Document) -> usize;
    fn may_select_points(&self, doc: &Document) -> bool;
    fn resolve(
        self: Box<Self>,
        doc: &Document,
        hits: Option<&Selection>,
    ) -> Result<Selection, Error>;
}

impl<T: IntoSelection> PlannedSelection for T {
    fn may_select_points(&self, doc: &Document) -> bool {
        IntoSelection::may_select_points(self, doc)
    }

    fn estimate_cost(&self, doc: &Document) -> usize {
        IntoSelection::estimate_cost(self, doc)
    }

    fn resolve(
        self: Box<Self>,
        doc: &Document,
        hits: Option<&Selection>,
    ) -> Result<Selection, Error> {
        match hits {
            Some(hits) => (*self).into_selection_within(doc, hits),
            None => (*self).into_selection(doc),
        }
    }
}

enum Step<'a> {
    Within(Box<dyn PlannedSelection + 'a>),
    Outside(Box<dyn PlannedSelection + 'a>),
    WithText(String, bool),
    FollowedBy(SpanFilter, Distance),
    PrecededBy(SpanFilter, Distance),
    ExpandTo(SpanFilter),
}

/// A document query that starts with the whole text and applies steps to narrow or widen the match ranges.
/// The steps run when a result is requested, for example with [`Query::get_selection`].
///
/// Between `expand_to` steps, selections narrow the search area first, followed
/// by text searches, then proximity filters. Selection work is ordered by
/// estimated index sizes, so a layer restriction applies even when it is
/// written after a text search. Text searches retain their relative order.
/// Selection steps that may contain points retain their place among the other
/// selection steps because inclusive point boundaries are order-sensitive.
pub struct Query<'a> {
    doc: &'a Document,
    steps: Vec<Step<'a>>,
    layers: Option<Vec<String>>,
}

impl<'a> Query<'a> {
    pub(crate) fn new(doc: &'a Document) -> Self {
        Self {
            doc,
            steps: Vec::new(),
            layers: None,
        }
    }

    fn push(mut self, step: Step<'a>) -> Self {
        self.steps.push(step);
        self
    }

    /// Keeps only the portions of the current hits within the given selection.
    pub fn within(self, selection: impl IntoSelection + 'a) -> Self {
        self.push(Step::Within(Box::new(selection)))
    }

    /// Removes the portions of the current hits within the given selection.
    pub fn outside(self, selection: impl IntoSelection + 'a) -> Self {
        self.push(Step::Outside(Box::new(selection)))
    }

    /// Keeps only the portions of the current hits covered by spans in the given layer.
    pub fn in_layer(self, layer: impl Into<String>) -> Self {
        self.within(spans_in(layer))
    }

    /// Keeps only the portions of the current hits covered by the specified span.
    pub fn in_span(self, layer: impl Into<String>, id: impl Into<SpanId>) -> Self {
        self.within(span_id(layer, id))
    }

    /// Finds literal occurrences of the text wholly within the current hit ranges.
    pub fn with_text(self, text: &str) -> Self {
        self.push(Step::WithText(text.into(), false))
    }

    /// Finds occurrences within the current hit ranges, ignoring case and nonspacing combining marks.
    pub fn with_text_folded(self, text: &str) -> Self {
        self.push(Step::WithText(text.into(), true))
    }

    /// Keeps hits followed by a matching span range within the given distance.
    pub fn followed_by(self, filter: impl Into<SpanFilter>, distance: Distance) -> Self {
        self.push(Step::FollowedBy(filter.into(), distance))
    }

    /// Keeps hits preceded by a matching span range within the given distance.
    pub fn preceded_by(self, filter: impl Into<SpanFilter>, distance: Distance) -> Self {
        self.push(Step::PrecededBy(filter.into(), distance))
    }

    /// Replaces the hits with the union of ranges from matching spans that touch a hit.
    pub fn expand_to(self, filter: impl Into<SpanFilter>) -> Self {
        self.push(Step::ExpandTo(filter.into()))
    }

    /// Limits the annotation layers included in passages without changing which text matches.
    pub fn with_layers<I, S>(mut self, layers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.layers = Some(layers.into_iter().map(Into::into).collect());
        self
    }

    fn execute(doc: &Document, mut steps: Vec<Step<'a>>) -> Result<Selection, Error> {
        let mut start = 0;
        for end in 0..=steps.len() {
            if end == steps.len() || matches!(steps[end], Step::ExpandTo(_)) {
                // Stable sorting preserves the order of text transformations.
                // Expansions are never moved across their surrounding steps.
                let block = &mut steps[start..end];
                block.sort_by_key(|step| match step {
                    Step::Within(_) | Step::Outside(_) => (0, 0),
                    Step::WithText(_, _) => (1, 0),
                    Step::FollowedBy(filter, distance) | Step::PrecededBy(filter, distance) => {
                        let intervening = match &distance.kind {
                            DistanceKind::AtMost(_, layer) => {
                                doc.estimate_filter_cost(&spans_in(layer))
                            }
                            _ => 0,
                        };
                        (
                            2,
                            doc.estimate_filter_cost(filter).saturating_add(intervening),
                        )
                    }
                    Step::ExpandTo(_) => unreachable!(),
                });
                let restrictions = block
                    .iter()
                    .position(|step| !matches!(step, Step::Within(_) | Step::Outside(_)))
                    .unwrap_or(block.len());
                let mut run_start = 0;
                for run_end in 0..=restrictions {
                    let boundary = run_end == restrictions
                        || match &block[run_end] {
                            Step::Within(selection) | Step::Outside(selection) => {
                                selection.may_select_points(doc)
                            }
                            _ => unreachable!(),
                        };
                    if boundary {
                        // Inclusive point boundaries make selection algebra
                        // order-sensitive. Never sort across a possible point
                        // selector or an unknown custom/source selector.
                        block[run_start..run_end].sort_by_key(|step| match step {
                            Step::Within(selection) | Step::Outside(selection) => {
                                selection.estimate_cost(doc)
                            }
                            _ => unreachable!(),
                        });
                        run_start = run_end + 1;
                    }
                }
                start = end + 1;
            }
        }
        let mut hits = Selection::from_ranges(alloc::vec![0..doc.len()]);
        for step in steps {
            hits = match step {
                Step::Within(selection) => selection.resolve(doc, Some(&hits))?,
                Step::Outside(selection) => hits.subtract(&selection.resolve(doc, None)?),
                Step::WithText(text, folded) => {
                    if folded {
                        find_text_folded(doc, &hits, &text)
                    } else {
                        find_text(doc, &hits, &text)
                    }
                }
                Step::FollowedBy(filter, distance) => {
                    matching_neighbours(doc, hits, &filter, &distance, true)?
                }
                Step::PrecededBy(filter, distance) => {
                    matching_neighbours(doc, hits, &filter, &distance, false)?
                }
                Step::ExpandTo(filter) => {
                    let mut ranges = Vec::new();
                    doc.visit_matching_ranges(&filter, |matched| {
                        if span_ranges_touch(matched, &hits) {
                            ranges.extend_from_slice(matched);
                        }
                    })?;
                    Selection::from_ranges(ranges)
                }
            };
        }
        Ok(hits)
    }

    /// Runs the query and returns its matching text ranges as a selection.
    pub fn get_selection(self) -> Result<Selection, Error> {
        Self::execute(self.doc, self.steps)
    }

    /// Runs the query and returns one passage per matching range.
    pub fn get_passages(self) -> Result<Vec<Passage>, Error> {
        let doc = self.doc;
        let layers = self.layers;
        let hits = Self::execute(doc, self.steps)?;
        hits.ranges()
            .iter()
            .cloned()
            .map(|range| doc.passage(range, layers.as_deref()))
            .collect()
    }

    /// Runs the query and returns a passage if there is exactly one matching range.
    pub fn get_passage(self) -> Result<Passage, Error> {
        let doc = self.doc;
        let layers = self.layers;
        let hits = Self::execute(doc, self.steps)?;
        if hits.ranges().len() != 1 {
            return Err(Error::NotOne {
                found: hits.ranges().len(),
            });
        }
        doc.passage(hits.ranges()[0].clone(), layers.as_deref())
    }

    /// Returns spans that match the filter and touch at least one hit.
    pub fn get_spans(self, filter: impl Into<SpanFilter>) -> Result<Vec<Span>, Error> {
        let doc = self.doc;
        let hits = Self::execute(doc, self.steps)?;
        let mut spans: Vec<MatchedSpan> = doc
            .matching_spans(&filter.into())?
            .into_iter()
            .filter(|span| span_ranges_touch(&span.ranges, &hits))
            .collect();
        spans.sort_by_key(|span| {
            (
                span.ranges.first().map_or(u32::MAX, |range| range.start),
                span.layer_index,
            )
        });
        Ok(spans.into_iter().map(|span| span.span).collect())
    }

    /// Returns the number of matching text ranges.
    pub fn count(self) -> Result<usize, Error> {
        Ok(Self::execute(self.doc, self.steps)?.ranges().len())
    }

    /// Counts spans that match the filter and touch at least one hit.
    pub fn count_spans(self, filter: impl Into<SpanFilter>) -> Result<usize, Error> {
        let doc = self.doc;
        let hits = Self::execute(doc, self.steps)?;
        doc.count_matching_spans_touching(&filter.into(), &hits)
    }

    /// Counts how many spans touching a hit have each value for the given layer and key.
    /// Returns the most frequent values first.
    pub fn count_values(self, layer: &str, key: &str) -> Result<Vec<(Value, usize)>, Error> {
        let doc = self.doc;
        let hits = Self::execute(doc, self.steps)?;
        let spans = doc.matching_spans(&spans_in(layer))?;
        let mut counts: Vec<(Value, usize)> = Vec::new();

        for matched in spans
            .iter()
            .filter(|span| span_ranges_touch(&span.ranges, &hits))
        {
            let mut seen = Vec::new();
            for value in matched.span.values(key) {
                if seen.contains(&value) {
                    continue;
                }
                seen.push(value);
                if let Some(index) = counts.iter().position(|(previous, _)| previous == value) {
                    counts[index].1 += 1;
                } else {
                    counts.push((value.clone(), 1));
                }
            }
        }
        counts.sort_by(|(left, left_count), (right, right_count)| {
            right_count
                .cmp(left_count)
                .then_with(|| compare_values(left, right))
        });
        Ok(counts)
    }
}

fn find_text(doc: &Document, hits: &Selection, text: &str) -> Selection {
    if text.is_empty() {
        return Selection::from_ranges(Vec::new());
    }
    let mut ranges = Vec::new();
    for hit in hits.ranges() {
        let fragment = doc.slice(hit.clone());
        ranges.extend(fragment.match_indices(text).map(|(offset, _)| {
            hit.start + offset as u32..hit.start + (offset + text.len()) as u32
        }));
    }
    Selection::from_ranges(ranges)
}

fn fold(input: &str, base: u32) -> Vec<(char, Range<u32>)> {
    let mut folded = Vec::new();
    for (offset, character) in input.char_indices() {
        let start = base + offset as u32;
        let end = start + character.len_utf8() as u32;
        let source = character.to_string();
        for decomposed in source.nfd() {
            if !is_combining_mark(decomposed) {
                for lower in decomposed.to_lowercase() {
                    folded.push((lower, start..end));
                }
            }
        }
    }
    folded
}

fn find_text_folded(doc: &Document, hits: &Selection, text: &str) -> Selection {
    let pattern: Vec<char> = fold(text, 0)
        .into_iter()
        .map(|(character, _)| character)
        .collect();
    if pattern.is_empty() {
        return Selection::from_ranges(Vec::new());
    }
    let mut ranges = Vec::new();
    for hit in hits.ranges() {
        let folded = fold(doc.slice(hit.clone()), hit.start);
        if pattern.len() > folded.len() {
            continue;
        }
        for start in 0..=folded.len() - pattern.len() {
            if folded[start..start + pattern.len()]
                .iter()
                .map(|(character, _)| *character)
                .eq(pattern.iter().copied())
            {
                ranges.push(folded[start].1.start..folded[start + pattern.len() - 1].1.end);
            }
        }
    }
    Selection::from_ranges(ranges)
}

fn matching_neighbours(
    doc: &Document,
    mut hits: Selection,
    filter: &SpanFilter,
    distance: &Distance,
    follows: bool,
) -> Result<Selection, Error> {
    let mut neighbours = Vec::new();
    doc.visit_matching_ranges(filter, |ranges| {
        neighbours.extend(ranges.iter().map(|r| if follows { r.start } else { r.end }));
    })?;
    let mut intervening = match &distance.kind {
        DistanceKind::AtMost(_, layer) => Some(SpanGapIndex::build(doc, layer)?),
        _ => None,
    };
    // Even empty hits must resolve the filters above to preserve query errors.
    if !neighbours.is_sorted() {
        neighbours.sort_unstable();
    }
    let mut cursor = 0;
    hits.retain_ranges(|hit| {
        // The closest eligible neighbour gives the smallest gap. If its gap
        // fails, any farther neighbour's superset must also fail.
        let gap = if follows {
            while cursor < neighbours.len() && neighbours[cursor] < hit.end {
                cursor += 1;
            }
            let Some(&start) = neighbours.get(cursor) else {
                return false;
            };
            hit.end..start
        } else {
            while cursor < neighbours.len() && neighbours[cursor] <= hit.start {
                cursor += 1;
            }
            if cursor == 0 {
                return false;
            }
            neighbours[cursor - 1]..hit.start
        };
        match &distance.kind {
            DistanceKind::Bytes(max) => gap.end - gap.start <= *max,
            DistanceKind::Adjacent => doc
                .slice(gap)
                .chars()
                .all(|ch| ch.is_whitespace() || is_punctuation(ch)),
            DistanceKind::AtMost(max, _) => intervening.as_mut().unwrap().at_most(gap, *max),
        }
    });
    Ok(hits)
}

struct GapEntry {
    range: Range<u32>,
    span: usize,
}

struct SpanGapIndex {
    entries: Vec<GapEntry>,
    first: usize,
    past_end: usize,
    ordered_ends: bool,
    // Present only if a span has multiple ranges and needs deduplication.
    seen: Vec<usize>,
    generation: usize,
}

impl SpanGapIndex {
    fn build(doc: &Document, layer: &str) -> Result<Self, Error> {
        let mut entries = Vec::new();
        let mut span = 0;
        let mut multi_range = false;
        doc.visit_matching_ranges(&spans_in(layer), |ranges| {
            multi_range |= ranges.len() > 1;
            entries.extend(ranges.iter().cloned().map(|range| GapEntry { range, span }));
            span += 1;
        })?;
        if !entries.is_sorted_by_key(|entry| entry.range.start) {
            entries.sort_unstable_by_key(|entry| entry.range.start);
        }
        let ordered_ends = !multi_range && entries.is_sorted_by_key(|entry| entry.range.end);
        Ok(Self {
            entries,
            first: 0,
            past_end: 0,
            ordered_ends,
            seen: if multi_range {
                alloc::vec![0; span]
            } else {
                Vec::new()
            },
            generation: 0,
        })
    }

    fn at_most(&mut self, gap: Range<u32>, max: usize) -> bool {
        // Both gap boundaries advance monotonically as hits advance.
        while self.first < self.entries.len() && self.entries[self.first].range.start < gap.start {
            self.first += 1;
        }
        if self.ordered_ends {
            // When starts and ends share the same order and each span has one
            // range, the contained spans are exactly a single slice.
            self.past_end = self.past_end.max(self.first);
            while self.past_end < self.entries.len()
                && self.entries[self.past_end].range.end <= gap.end
            {
                self.past_end += 1;
            }
            return self.past_end - self.first <= max;
        }

        self.generation += 1;
        let mut count = 0;
        for entry in &self.entries[self.first..] {
            if entry.range.start > gap.end {
                break;
            }
            if entry.range.end <= gap.end {
                if let Some(seen) = self.seen.get_mut(entry.span) {
                    if *seen == self.generation {
                        continue;
                    }
                    *seen = self.generation;
                }
                count += 1;
                if count > max {
                    return false;
                }
            }
        }
        true
    }
}

fn is_punctuation(character: char) -> bool {
    matches!(
        get_general_category(character),
        GeneralCategory::ConnectorPunctuation
            | GeneralCategory::DashPunctuation
            | GeneralCategory::OpenPunctuation
            | GeneralCategory::ClosePunctuation
            | GeneralCategory::InitialPunctuation
            | GeneralCategory::FinalPunctuation
            | GeneralCategory::OtherPunctuation
    )
}

fn compare_values(left: &Value, right: &Value) -> Ordering {
    use Value::{Bool, Float, Int, Null, Str};
    match (left, right) {
        (Null, Null) => Ordering::Equal,
        (Null, _) => Ordering::Less,
        (_, Null) => Ordering::Greater,
        (Bool(left), Bool(right)) => left.cmp(right),
        (Bool(_), _) => Ordering::Less,
        (_, Bool(_)) => Ordering::Greater,
        (Int(left), Int(right)) => left.cmp(right),
        (Float(left), Float(right)) => left.total_cmp(right),
        (Int(left), Float(right)) => (*left as f64).total_cmp(right),
        (Float(left), Int(right)) => left.total_cmp(&(*right as f64)),
        (Int(_) | Float(_), Str(_)) => Ordering::Less,
        (Str(_), Int(_) | Float(_)) => Ordering::Greater,
        (Str(left), Str(right)) => left.cmp(right),
    }
}

#[derive(Clone, Debug)]
enum DistanceKind {
    Adjacent,
    AtMost(usize, String),
    Bytes(u32),
}

/// How far apart the two parts of `followed_by` and `preceded_by` may be.
#[derive(Clone, Debug)]
pub struct Distance {
    kind: DistanceKind,
}

impl Distance {
    /// Only whitespace and punctuation in between.
    pub fn adjacent() -> Self {
        Self {
            kind: DistanceKind::Adjacent,
        }
    }

    /// At most `n` spans of `layer` lie wholly in between.
    pub fn at_most(n: usize, layer: impl Into<String>) -> Self {
        Self {
            kind: DistanceKind::AtMost(n, layer.into()),
        }
    }

    /// At most `n` bytes in between.
    pub fn bytes(n: u32) -> Self {
        Self {
            kind: DistanceKind::Bytes(n),
        }
    }
}
