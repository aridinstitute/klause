use alloc::{string::String, vec::Vec};
use core::marker::PhantomData;

use crate::document::{Document, Error, Span, SpanId, Value};
use crate::passage::Passage;
use crate::selection::{IntoSelection, Selection, SpanFilter};

/// Starts from the whole text, narrows or widens it step by step, and ends with a verb.
pub struct Query<'a> {
    _doc: PhantomData<&'a Document>,
}

impl<'a> Query<'a> {
    pub fn within(self, _selection: impl IntoSelection + 'a) -> Self {
        todo!()
    }

    pub fn outside(self, _selection: impl IntoSelection + 'a) -> Self {
        todo!()
    }

    pub fn in_layer(self, _layer: impl Into<String>) -> Self {
        todo!()
    }

    pub fn in_span(self, _layer: impl Into<String>, _id: impl Into<SpanId>) -> Self {
        todo!()
    }

    pub fn with_text(self, _text: &str) -> Self {
        todo!()
    }

    pub fn with_text_folded(self, _text: &str) -> Self {
        todo!()
    }

    pub fn followed_by(self, _filter: impl Into<SpanFilter>, _distance: Distance) -> Self {
        todo!()
    }

    pub fn preceded_by(self, _filter: impl Into<SpanFilter>, _distance: Distance) -> Self {
        todo!()
    }

    pub fn expand_to(self, _filter: impl Into<SpanFilter>) -> Self {
        todo!()
    }

    pub fn with_layers<I, S>(self, _layers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        todo!()
    }

    pub fn get_selection(self) -> Result<Selection, Error> {
        todo!()
    }

    pub fn get_passages(self) -> Result<Vec<Passage>, Error> {
        todo!()
    }

    pub fn get_passage(self) -> Result<Passage, Error> {
        todo!()
    }

    pub fn get_spans(self, _filter: impl Into<SpanFilter>) -> Result<Vec<Span>, Error> {
        todo!()
    }

    pub fn count(self) -> Result<usize, Error> {
        todo!()
    }

    pub fn count_spans(self, _filter: impl Into<SpanFilter>) -> Result<usize, Error> {
        todo!()
    }

    pub fn count_values(self, _layer: &str, _key: &str) -> Result<Vec<(Value, usize)>, Error> {
        todo!()
    }
}

/// How far apart the two parts of `followed_by` and `preceded_by` may be.
#[derive(Clone, Debug)]
pub struct Distance {
    _private: (),
}

impl Distance {
    /// Only whitespace and punctuation in between.
    pub fn adjacent() -> Self {
        todo!()
    }

    /// At most `n` spans of `layer` lie wholly in between.
    pub fn at_most(_n: usize, _layer: impl Into<String>) -> Self {
        todo!()
    }

    /// At most `n` bytes in between.
    pub fn bytes(_n: u32) -> Self {
        todo!()
    }
}
