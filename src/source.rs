use alloc::vec::Vec;
use core::{fmt, ops::Range};

use crate::document::Span;
use crate::selection::SpanFilter;

/// Reads a layer's spans from the application's own storage every time they are needed.
pub trait SpanSource: Send + Sync {
    /// Every span that touches `range`.
    fn get_spans(&self, range: Range<u32>) -> Result<Vec<Span>, SourceError>;

    /// Answers a filter on this layer directly, so a query doesn't call `get_spans` over the
    /// whole text.
    fn find_spans(&self, _filter: &SpanFilter) -> Option<Result<Vec<Span>, SourceError>> {
        None
    }
}

/// An error reported by a `SpanSource`.
#[derive(Debug)]
pub struct SourceError {
    _private: (),
}

impl SourceError {
    pub fn new(_error: impl core::error::Error + Send + Sync + 'static) -> Self {
        todo!()
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl core::error::Error for SourceError {}
