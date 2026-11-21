#![cfg_attr(not(feature = "std"), no_std)]

//! Klause stores and queries annotated text.
//!
//! A [`Document`] holds text and annotation layers. A [`Span`] is an annotation
//! that targets text ranges or another span; a [`Selection`] is just a set of
//! text ranges. A [`Passage`] gives you the text and annotations for one range.
//!
//! The API examples use Genesis 3:15, often starting from the excerpt
//! “And I will put enmity”. In that excerpt, the byte range `15..21` is “enmity”.
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
