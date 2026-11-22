use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::{cmp::Ordering, ops::Range};
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::document::{Document, Error, MatchedSpan, Span, SpanId, Value};
use crate::passage::Passage;
use crate::selection::{IntoSelection, Selection, SpanFilter, span_id, spans_in};

enum Step {
    Within(Result<Selection, Error>),
    Outside(Result<Selection, Error>),
    WithText(String, bool),
    FollowedBy(SpanFilter, Distance),
    PrecededBy(SpanFilter, Distance),
    ExpandTo(SpanFilter),
}

/// A document query that starts with the whole text and applies steps to narrow or widen the match ranges.
/// The steps run when a result is requested, for example with [`Query::get_selection`].
pub struct Query<'a> {
    doc: &'a Document,
    steps: Vec<Step>,
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

    fn push(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Keeps only the portions of the current hits within the given selection.
    pub fn within(self, selection: impl IntoSelection + 'a) -> Self {
        let result = selection.into_selection(self.doc);
        self.push(Step::Within(result))
    }

    /// Removes the portions of the current hits within the given selection.
    pub fn outside(self, selection: impl IntoSelection + 'a) -> Self {
        let result = selection.into_selection(self.doc);
        self.push(Step::Outside(result))
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

    fn execute(doc: &Document, steps: Vec<Step>) -> Result<Selection, Error> {
        let mut hits = Selection::from_ranges(alloc::vec![0..doc.len()]);
        for step in steps {
            hits = match step {
                Step::Within(selection) => hits.intersect(&selection?),
                Step::Outside(selection) => hits.subtract(&selection?),
                Step::WithText(text, folded) => {
                    if folded {
                        find_text_folded(doc, &hits, &text)
                    } else {
                        find_text(doc, &hits, &text)
                    }
                }
                Step::FollowedBy(filter, distance) => {
                    let spans = doc.matching_spans(&filter)?;
                    let intervening = intervening_spans(doc, &distance)?;
                    matching_neighbours(doc, &hits, &spans, &distance, &intervening, true)
                }
                Step::PrecededBy(filter, distance) => {
                    let spans = doc.matching_spans(&filter)?;
                    let intervening = intervening_spans(doc, &distance)?;
                    matching_neighbours(doc, &hits, &spans, &distance, &intervening, false)
                }
                Step::ExpandTo(filter) => {
                    let spans = doc.matching_spans(&filter)?;
                    let ranges = spans
                        .into_iter()
                        .filter(|span| span_touches_hits(&span.ranges, &hits))
                        .flat_map(|span| span.ranges)
                        .collect();
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
            .filter(|span| span_touches_hits(&span.ranges, &hits))
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
            .filter(|span| span_touches_hits(&span.ranges, &hits))
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

fn span_touches_hits(ranges: &[Range<u32>], hits: &Selection) -> bool {
    ranges.iter().any(|span| {
        hits.ranges().iter().any(|hit| {
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

fn matching_neighbours(
    doc: &Document,
    hits: &Selection,
    candidates: &[MatchedSpan],
    distance: &Distance,
    intervening: &[MatchedSpan],
    follows: bool,
) -> Selection {
    let ranges = hits
        .ranges()
        .iter()
        .filter(|hit| {
            candidates.iter().any(|candidate| {
                candidate.ranges.iter().any(|candidate_range| {
                    let gap = if follows {
                        if candidate_range.start < hit.end {
                            return false;
                        }
                        hit.end..candidate_range.start
                    } else {
                        if candidate_range.end > hit.start {
                            return false;
                        }
                        candidate_range.end..hit.start
                    };
                    within_distance(doc, &gap, distance, intervening)
                })
            })
        })
        .cloned()
        .collect();
    Selection::from_ranges(ranges)
}

fn within_distance(
    doc: &Document,
    gap: &Range<u32>,
    distance: &Distance,
    intervening: &[MatchedSpan],
) -> bool {
    match &distance.kind {
        DistanceKind::Bytes(max) => gap.end - gap.start <= *max,
        DistanceKind::Adjacent => doc
            .slice(gap.clone())
            .chars()
            .all(|character| character.is_whitespace() || is_punctuation(character)),
        DistanceKind::AtMost(max, _) => {
            let count = intervening
                .iter()
                .filter(|span| {
                    span.ranges
                        .iter()
                        .any(|range| range.start >= gap.start && range.end <= gap.end)
                })
                .count();
            count <= *max
        }
    }
}

fn intervening_spans(doc: &Document, distance: &Distance) -> Result<Vec<MatchedSpan>, Error> {
    match &distance.kind {
        DistanceKind::AtMost(_, layer) => doc.matching_spans(&spans_in(layer)),
        DistanceKind::Bytes(_) | DistanceKind::Adjacent => Ok(Vec::new()),
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
