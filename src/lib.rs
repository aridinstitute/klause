#![cfg_attr(not(feature = "std"), no_std)]

//! Klause — a short description of the library.
//!
//! See the [README](https://github.com/aridinstitute/klause#readme) for an
//! overview and usage examples.

extern crate alloc;

mod document;
mod layer;
mod passage;
mod query;
mod selection;
mod source;

pub use document::{
    Document, Error, IntoDocument, IntoValues, Span, SpanId, SpanRef, Target, Value,
};
pub use layer::Layer;
pub use passage::{Passage, PassageSpan};
pub use query::{Distance, Query};
pub use selection::{
    AnyOf, IntoSelection, Selection, SpanFilter, ValueMatcher, any_of, select_all_spans,
    select_span, select_spans,
};
pub use source::{SourceError, SpanSource};
