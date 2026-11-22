use alloc::{boxed::Box, vec::Vec};
use core::{fmt, ops::Range};

use crate::document::Span;
use crate::selection::SpanFilter;

/// Reads a layer's spans from the application's own storage every time they are needed.
pub trait SpanSource: Send + Sync {
    /// Every span that touches `range`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{SourceError, Span, SpanSource};
    /// # struct EnmityHighlight;
    /// # impl SpanSource for EnmityHighlight {
    /// #     fn get_spans(&self, range: std::ops::Range<u32>) -> Result<Vec<Span>, SourceError> {
    /// #         Ok(if range.start < 21 && 15 < range.end {
    /// #             vec![Span::from(15..21).with_attr("topic", "enmity")]
    /// #         } else {
    /// #             Vec::new()
    /// #         })
    /// #     }
    /// # }
    /// let highlights = EnmityHighlight.get_spans(15..21)?;
    /// assert_eq!(highlights.len(), 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn get_spans(&self, range: Range<u32>) -> Result<Vec<Span>, SourceError>;

    /// Optionally resolves a filter directly against this source.
    /// Return `None` to let the query load spans with [`SpanSource::get_spans`] instead.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::{SourceError, Span, SpanSource, spans_in};
    /// # struct EnmityHighlight;
    /// # impl SpanSource for EnmityHighlight {
    /// #     fn get_spans(&self, range: std::ops::Range<u32>) -> Result<Vec<Span>, SourceError> {
    /// #         Ok(if range.start < 21 && 15 < range.end {
    /// #             vec![Span::from(15..21).with_attr("topic", "enmity")]
    /// #         } else {
    /// #             Vec::new()
    /// #         })
    /// #     }
    /// # }
    /// let nouns = spans_in("highlights").with_attr("topic", "enmity");
    /// // This source uses the default fallback to range-based loading.
    /// assert!(EnmityHighlight.find_spans(&nouns).is_none());
    /// ```
    fn find_spans(&self, _filter: &SpanFilter) -> Option<Result<Vec<Span>, SourceError>> {
        None
    }
}

/// An error reported by a `SpanSource`.
#[derive(Debug)]
pub struct SourceError {
    error: Box<dyn core::error::Error + Send + Sync>,
}

impl SourceError {
    /// Wraps an error from the underlying span source.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use klause::SourceError;
    /// let err = std::io::Error::other("could not load highlights for Genesis 3:15");
    /// let source_error = SourceError::new(err);
    /// ```
    pub fn new(error: impl core::error::Error + Send + Sync + 'static) -> Self {
        Self {
            error: Box::new(error),
        }
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl core::error::Error for SourceError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(&*self.error)
    }
}
