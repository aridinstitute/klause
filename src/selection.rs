use alloc::{string::String, vec::Vec};
use core::ops::{Range, RangeFrom, RangeFull};

use crate::document::{Document, Error, SpanId, SpanRef, Value};

/// A sorted list of non-overlapping byte ranges of a document's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    _private: (),
}

impl Selection {
    pub fn ranges(&self) -> &[Range<u32>] {
        todo!()
    }

    pub fn union(&self, _other: &Selection) -> Selection {
        todo!()
    }

    pub fn intersect(&self, _other: &Selection) -> Selection {
        todo!()
    }

    pub fn subtract(&self, _other: &Selection) -> Selection {
        todo!()
    }

    pub fn contains(&self, _position: u32) -> bool {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        todo!()
    }
}

/// Text described against a document, resolved when a query step or passage runs.
pub trait IntoSelection {
    fn into_selection(self, doc: &Document) -> Result<Selection, Error>;
}

macro_rules! into_selection {
    ($($source:ty),*) => {$(
        impl IntoSelection for $source {
            fn into_selection(self, _doc: &Document) -> Result<Selection, Error> {
                todo!()
            }
        }
    )*};
}

into_selection!(
    Range<u32>,
    RangeFull,
    Vec<Range<u32>>,
    Selection,
    &Selection,
    SpanRef,
    SpanFilter
);

impl<const N: usize> IntoSelection for [Range<u32>; N] {
    fn into_selection(self, _doc: &Document) -> Result<Selection, Error> {
        todo!()
    }
}

/// A description of spans, such as "spans in `morph` whose `pos` is `verb`".
#[derive(Clone, Debug)]
pub struct SpanFilter {
    _private: (),
}

impl SpanFilter {
    pub fn with_attr(self, _key: impl Into<String>, _matcher: impl ValueMatcher) -> Self {
        todo!()
    }

    pub fn without_attr(self, _key: impl Into<String>, _value: impl Into<Value>) -> Self {
        todo!()
    }

    pub fn with_key(self, _key: impl Into<String>) -> Self {
        todo!()
    }

    pub fn without_key(self, _key: impl Into<String>) -> Self {
        todo!()
    }

    pub fn with_id(self, _id: impl Into<SpanId>) -> Self {
        todo!()
    }

    pub fn targeting(self, _target: SpanFilter) -> Self {
        todo!()
    }
}

impl From<&str> for SpanFilter {
    fn from(_layer: &str) -> Self {
        todo!()
    }
}

/// What an attribute filter accepts: a value, `any_of` several values, or a numeric range.
pub trait ValueMatcher {
    fn matches(&self, value: &Value) -> bool;
}

macro_rules! value_matcher {
    ($($source:ty),*) => {$(
        impl ValueMatcher for $source {
            fn matches(&self, _value: &Value) -> bool {
                todo!()
            }
        }
    )*};
}

value_matcher!(
    Value,
    &str,
    String,
    i32,
    i64,
    f64,
    bool,
    AnyOf,
    Range<i32>,
    RangeFrom<i32>,
    Range<i64>,
    RangeFrom<i64>,
    Range<f64>,
    RangeFrom<f64>
);

/// Matches any of several values.
#[derive(Clone, Debug)]
pub struct AnyOf {
    _private: (),
}

pub fn any_of<T: Into<Value>>(_values: impl IntoIterator<Item = T>) -> AnyOf {
    todo!()
}

pub fn select_span(_layer: impl Into<String>, _id: impl Into<SpanId>) -> SpanRef {
    todo!()
}

pub fn select_spans(_layer: impl Into<String>) -> SpanFilter {
    todo!()
}

pub fn select_all_spans() -> SpanFilter {
    todo!()
}
