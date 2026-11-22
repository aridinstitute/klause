use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use core::fmt;

use crate::query::ValueCounts;
use crate::{
    Distance, Document, Error, IntoSelection, Passage, Query, Selection, Span, SpanFilter, SpanId,
    Value, span_id, spans_in,
};

/// An in-memory collection of documents with unique IDs.
///
/// Queries run independently on each selected document. Byte offsets and span
/// targets always belong to one document; nothing matches across a boundary.
#[derive(Debug, Default)]
pub struct Corpus {
    documents: BTreeMap<String, Document>,
}

impl Corpus {
    /// Creates an empty corpus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a document without replacing an existing document with the same ID.
    pub fn add_document(&mut self, document: Document) -> Result<(), CorpusError> {
        use alloc::collections::btree_map::Entry;

        match self.documents.entry(document.id().into()) {
            Entry::Vacant(entry) => {
                entry.insert(document);
                Ok(())
            }
            Entry::Occupied(entry) => Err(CorpusError::DuplicateDocument(entry.key().clone())),
        }
    }

    /// Borrows the document with this ID, if present.
    pub fn document(&self, id: &str) -> Option<&Document> {
        self.documents.get(id)
    }

    /// Removes and returns the document with this ID, if present.
    pub fn remove_document(&mut self, id: &str) -> Option<Document> {
        self.documents.remove(id)
    }

    /// Iterates over documents in ascending ID order.
    pub fn documents(&self) -> impl ExactSizeIterator<Item = &Document> {
        self.documents.values()
    }

    /// Starts a query over all documents in this corpus.
    pub fn find(&self) -> CorpusQuery<'_> {
        CorpusQuery {
            corpus: self,
            ids: None,
            document_filters: Vec::new(),
            steps: Vec::new(),
        }
    }
}

type DocumentFilter<'a> = Box<dyn Fn(&Document) -> bool + 'a>;
type QueryStep<'a> = Box<dyn Fn(Query<'a>) -> Query<'a> + 'a>;

/// A query that applies the same steps independently to each selected document.
///
/// Document filters run before text or span steps, regardless of builder order.
/// Results follow ascending document ID order, then the usual order within each
/// document. An error in a document query aborts the corpus query and reports the
/// document ID. In particular, a required layer missing from a selected document
/// is an error; use [`CorpusQuery::filter_documents`] to skip such documents.
pub struct CorpusQuery<'a> {
    corpus: &'a Corpus,
    ids: Option<BTreeSet<String>>,
    document_filters: Vec<DocumentFilter<'a>>,
    steps: Vec<QueryStep<'a>>,
}

impl<'a> CorpusQuery<'a> {
    /// Sets the eligible document IDs, replacing an earlier list. Repeated IDs
    /// are ignored. If any ID is unknown, the final verb returns an error.
    pub fn in_documents(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.ids = Some(ids.into_iter().map(Into::into).collect());
        self
    }

    /// Narrows the eligible documents to those satisfying the predicate.
    /// Multiple predicates must all match. Inspect [`Document::values`] to filter
    /// by metadata or [`Document::layer`] to cope with heterogeneous layers.
    pub fn filter_documents(mut self, predicate: impl Fn(&Document) -> bool + 'a) -> Self {
        self.document_filters.push(Box::new(predicate));
        self
    }

    fn step(mut self, apply: impl Fn(Query<'a>) -> Query<'a> + 'a) -> Self {
        self.steps.push(Box::new(apply));
        self
    }

    /// Keeps the portions of the hits within a selection resolved in each document.
    pub fn within(self, selection: impl IntoSelection + Clone + 'a) -> Self {
        self.step(move |query| query.within(selection.clone()))
    }

    /// Removes the portions of the hits inside a selection resolved in each document.
    pub fn outside(self, selection: impl IntoSelection + Clone + 'a) -> Self {
        self.step(move |query| query.outside(selection.clone()))
    }

    /// Keeps the portions of the hits covered by the named layer in each document.
    pub fn in_layer(self, layer: impl Into<String>) -> Self {
        self.within(spans_in(layer))
    }

    /// Keeps the portions of the hits covered by the named span in each document.
    pub fn in_span(self, layer: impl Into<String>, id: impl Into<SpanId>) -> Self {
        self.within(span_id(layer, id))
    }

    /// Finds literal occurrences wholly within each document's hit ranges.
    pub fn with_text(self, text: &str) -> Self {
        let text = String::from(text);
        self.step(move |query| query.with_text(&text))
    }

    /// Finds occurrences using Unicode lowercase and ignoring combining marks.
    pub fn with_text_folded(self, text: &str) -> Self {
        let text = String::from(text);
        self.step(move |query| query.with_text_folded(&text))
    }

    /// Keeps hits followed by a matching span range in the same document.
    pub fn followed_by(self, filter: impl Into<SpanFilter>, distance: Distance) -> Self {
        let filter = filter.into();
        self.step(move |query| query.followed_by(filter.clone(), distance.clone()))
    }

    /// Keeps hits preceded by a matching span range in the same document.
    pub fn preceded_by(self, filter: impl Into<SpanFilter>, distance: Distance) -> Self {
        let filter = filter.into();
        self.step(move |query| query.preceded_by(filter.clone(), distance.clone()))
    }

    /// Expands hits to the ranges of touching spans within each document.
    pub fn expand_to(self, filter: impl Into<SpanFilter>) -> Self {
        let filter = filter.into();
        self.step(move |query| query.expand_to(filter.clone()))
    }

    /// Limits the layers loaded for passages without changing which text matches.
    pub fn with_layers(self, layers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let layers: Vec<String> = layers.into_iter().map(Into::into).collect();
        self.step(move |query| query.with_layers(layers.iter().cloned()))
    }

    fn for_each(
        self,
        mut run: impl FnMut(&'a Document, Query<'a>) -> Result<(), Error>,
    ) -> Result<(), CorpusError> {
        let visit = |doc: &'a Document| {
            if self.document_filters.iter().all(|filter| filter(doc)) {
                let query = self
                    .steps
                    .iter()
                    .fold(doc.find(), |query, step| step(query));
                run(doc, query).map_err(|error| CorpusError::Query {
                    document: doc.id().into(),
                    error,
                })?;
            }
            Ok(())
        };

        if let Some(ids) = self.ids {
            // Validate all explicit IDs before consulting any document sources.
            let documents = ids
                .into_iter()
                .map(|id| {
                    self.corpus
                        .document(&id)
                        .ok_or(CorpusError::UnknownDocument(id))
                })
                .collect::<Result<Vec<_>, _>>()?;
            documents.into_iter().try_for_each(visit)
        } else {
            self.corpus.documents().try_for_each(visit)
        }
    }

    /// Returns each nonempty selection paired with its document ID.
    pub fn get_selection(self) -> Result<Vec<(&'a str, Selection)>, CorpusError> {
        let mut selections = Vec::new();
        self.for_each(|doc, query| {
            let hits = query.get_selection()?;
            if !hits.is_empty() {
                selections.push((doc.id(), hits));
            }
            Ok(())
        })?;
        Ok(selections)
    }

    /// Returns a passage for each matching range, paired with its document ID.
    pub fn get_passages(self) -> Result<Vec<(&'a str, Passage)>, CorpusError> {
        let mut passages = Vec::new();
        self.for_each(|doc, query| {
            passages.extend(
                query
                    .get_passages()?
                    .into_iter()
                    .map(|passage| (doc.id(), passage)),
            );
            Ok(())
        })?;
        Ok(passages)
    }

    /// Returns spans that match the filter and touch a hit in their document.
    pub fn get_spans(
        self,
        filter: impl Into<SpanFilter>,
    ) -> Result<Vec<CorpusSpan<'a>>, CorpusError> {
        let filter = filter.into();
        let mut spans = Vec::new();
        self.for_each(|doc, query| {
            spans.extend(
                query
                    .matched_spans(&filter)?
                    .into_iter()
                    .map(|matched| CorpusSpan {
                        document_id: doc.id(),
                        layer: doc.layers().nth(matched.layer_index).unwrap().id(),
                        span: matched.span,
                    }),
            );
            Ok(())
        })?;
        Ok(spans)
    }

    /// Counts matching ranges across the selected documents.
    pub fn count(self) -> Result<usize, CorpusError> {
        let mut count = 0;
        self.for_each(|_, query| {
            count += query.count()?;
            Ok(())
        })?;
        Ok(count)
    }

    /// Counts matching spans that touch any hit in their own document.
    pub fn count_spans(self, filter: impl Into<SpanFilter>) -> Result<usize, CorpusError> {
        let filter = filter.into();
        let mut count = 0;
        self.for_each(|_, query| {
            count += query.count_spans_matching(&filter)?;
            Ok(())
        })?;
        Ok(count)
    }

    /// Counts touching spans per distinct value across the selected documents.
    /// Each span contributes at most once per equal value, even if it touches
    /// several hits. Results are sorted by descending count, then value order.
    pub fn count_values(self, layer: &str, key: &str) -> Result<Vec<(Value, usize)>, CorpusError> {
        let mut counts = ValueCounts::default();
        self.for_each(|_, query| query.count_values_into(layer, key, &mut counts))?;
        Ok(counts.finish())
    }
}

/// A query result identified by its document and layer, together with the span data.
#[derive(Clone, Debug)]
pub struct CorpusSpan<'a> {
    /// ID of the document the span came from.
    pub document_id: &'a str,
    /// ID of the layer the span came from.
    pub layer: &'a str,
    /// The matching span, whose target is local to the document.
    pub span: Span,
}

/// An error from adding documents or running a corpus query.
#[derive(Debug)]
pub enum CorpusError {
    /// A requested document ID does not exist.
    UnknownDocument(String),
    /// A document with this ID already exists.
    DuplicateDocument(String),
    /// A query failed inside this document.
    Query {
        /// ID of the document whose query failed.
        document: String,
        /// The underlying error.
        error: Error,
    },
}

impl fmt::Display for CorpusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDocument(id) => write!(f, "unknown document: {id}"),
            Self::DuplicateDocument(id) => write!(f, "duplicate document ID: {id}"),
            Self::Query { document, error } => write!(f, "document {document}: {error}"),
        }
    }
}

impl core::error::Error for CorpusError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Query { error, .. } => Some(error),
            _ => None,
        }
    }
}
