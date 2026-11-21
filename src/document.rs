use alloc::{string::String, vec::Vec};
use core::{fmt, ops::Range};

use crate::layer::Layer;
use crate::passage::Passage;
use crate::query::Query;
use crate::selection::{IntoSelection, Selection};
use crate::source::SourceError;

// Give iterator stubs a concrete return type without choosing storage yet.
pub(crate) fn pending_iter<T>() -> core::iter::Empty<T> {
    todo!()
}

#[derive(Debug)]
pub struct Document {
    _private: (),
}

impl Document {
    pub fn new(_id: impl Into<String>) -> Self {
        todo!()
    }

    pub fn with_attr(self, _key: impl Into<String>, _values: impl IntoValues) -> Self {
        todo!()
    }

    pub fn set_attr(self, _key: impl Into<String>, _values: impl IntoValues) -> Self {
        todo!()
    }

    pub fn remove_attr(self, _key: &str) -> Self {
        todo!()
    }

    pub fn with_layer(self, _layer: impl Into<Layer>) -> Self {
        todo!()
    }

    pub fn add_layer(&mut self, _layer: impl Into<Layer>) -> Result<(), Error> {
        todo!()
    }

    pub fn push(&mut self, _text: &str) -> Result<Range<u32>, Error> {
        todo!()
    }

    pub fn push_span(
        &mut self,
        _layer: &str,
        _text: &str,
        _build: impl FnOnce(Span) -> Span,
    ) -> Result<Range<u32>, Error> {
        todo!()
    }

    pub fn add_span(&mut self, _layer: &str, _span: Span) -> Result<SpanRef, Error> {
        todo!()
    }

    pub fn replace_span(&mut self, _reference: &SpanRef, _span: Span) -> Result<(), Error> {
        todo!()
    }

    pub fn remove_span(&mut self, _reference: &SpanRef) -> Result<(), Error> {
        todo!()
    }

    pub fn get_selection(&self, _selection: impl IntoSelection) -> Result<Selection, Error> {
        todo!()
    }

    pub fn get_passage(&self, _selection: impl IntoSelection) -> Result<Passage, Error> {
        todo!()
    }

    pub fn get_passages(&self, _selection: impl IntoSelection) -> Result<Vec<Passage>, Error> {
        todo!()
    }

    pub fn find(&self) -> Query<'_> {
        todo!()
    }

    pub fn id(&self) -> &str {
        todo!()
    }

    pub fn len(&self) -> u32 {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        todo!()
    }

    pub fn text(&self) -> &str {
        todo!()
    }

    pub fn slice(&self, _range: Range<u32>) -> &str {
        todo!()
    }

    pub fn attr(&self, _key: &str) -> Option<&Value> {
        todo!()
    }

    pub fn attrs(&self) -> impl Iterator<Item = (&str, &Value)> {
        pending_iter()
    }

    pub fn values(&self, _key: &str) -> impl Iterator<Item = &Value> {
        pending_iter()
    }

    pub fn layer(&self, _id: &str) -> Option<&Layer> {
        todo!()
    }

    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        pending_iter()
    }

    pub fn span(&self, _reference: &SpanRef) -> Option<&Span> {
        todo!()
    }

    pub fn ranges(&self, _reference: &SpanRef) -> Option<&[Range<u32>]> {
        todo!()
    }
}

pub trait IntoDocument {
    type Error: From<Error>;

    fn into_document(self) -> Result<Document, Self::Error>;
}

impl IntoDocument for Document {
    type Error = Error;

    fn into_document(self) -> Result<Document, Self::Error> {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub struct Span {
    _private: (),
}

impl Span {
    pub fn on(_layer: impl Into<String>, _id: impl Into<SpanId>) -> Self {
        todo!()
    }

    pub fn on_ref(_reference: &SpanRef) -> Self {
        todo!()
    }

    pub fn with_id(self, _id: impl Into<SpanId>) -> Self {
        todo!()
    }

    pub fn with_attr(self, _key: impl Into<String>, _values: impl IntoValues) -> Self {
        todo!()
    }

    pub fn set_attr(self, _key: impl Into<String>, _values: impl IntoValues) -> Self {
        todo!()
    }

    pub fn remove_attr(self, _key: &str) -> Self {
        todo!()
    }

    pub fn id(&self) -> Option<&SpanId> {
        todo!()
    }

    pub fn target(&self) -> &Target {
        todo!()
    }

    pub fn attr(&self, _key: &str) -> Option<&Value> {
        todo!()
    }

    pub fn values(&self, _key: &str) -> impl Iterator<Item = &Value> {
        pending_iter()
    }

    pub fn attrs(&self) -> impl Iterator<Item = (&str, &Value)> {
        pending_iter()
    }
}

impl From<Range<u32>> for Span {
    fn from(_range: Range<u32>) -> Self {
        todo!()
    }
}

impl From<Vec<Range<u32>>> for Span {
    fn from(_ranges: Vec<Range<u32>>) -> Self {
        todo!()
    }
}

impl<const N: usize> From<[Range<u32>; N]> for Span {
    fn from(_ranges: [Range<u32>; N]) -> Self {
        todo!()
    }
}

#[derive(Debug)]
pub enum Target {
    Text(Vec<Range<u32>>),
    Span(SpanRef),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanId {
    Int(u64),
    Str(String),
}

impl From<u64> for SpanId {
    fn from(_id: u64) -> Self {
        todo!()
    }
}

impl From<&str> for SpanId {
    fn from(_id: &str) -> Self {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanRef {
    _private: (),
}

impl SpanRef {
    pub fn new(_layer: impl Into<String>, _id: impl Into<SpanId>) -> Self {
        todo!()
    }

    pub fn layer(&self) -> &str {
        todo!()
    }

    pub fn id(&self) -> &SpanId {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl PartialEq for Value {
    fn eq(&self, _other: &Self) -> bool {
        todo!()
    }
}

impl From<&str> for Value {
    fn from(_value: &str) -> Self {
        todo!()
    }
}

macro_rules! value_from {
    ($($source:ty),*) => {$(
        impl From<$source> for Value {
            fn from(_value: $source) -> Self {
                todo!()
            }
        }
    )*};
}

value_from!(String, i32, i64, u32, f64, bool);

/// One or more attribute values: a value, or an array or `Vec` of values.
pub trait IntoValues {
    fn into_values(self) -> Vec<Value>;
}

macro_rules! into_values {
    ($($source:ty),*) => {$(
        impl IntoValues for $source {
            fn into_values(self) -> Vec<Value> {
                todo!()
            }
        }
    )*};
}

into_values!(Value, &str, String, i32, i64, u32, f64, bool);

impl<T: Into<Value>, const N: usize> IntoValues for [T; N] {
    fn into_values(self) -> Vec<Value> {
        todo!()
    }
}

impl<T: Into<Value>> IntoValues for Vec<T> {
    fn into_values(self) -> Vec<Value> {
        todo!()
    }
}

#[derive(Debug)]
pub enum Error {
    UnknownLayer(String),
    DuplicateLayer(String),
    UnknownSpan(SpanRef),
    DuplicateId(SpanRef),
    Cycle(SpanRef),
    ReadOnlyLayer(String),
    OutOfRange(Range<u32>),
    NotCharBoundary(u32),
    SplitsCharacter,
    TooLong,
    NotOne { found: usize },
    Source { layer: String, error: SourceError },
}

impl fmt::Display for Error {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl core::error::Error for Error {}
