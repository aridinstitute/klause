use alloc::vec::Vec;
use core::ops::{Deref, Range};

use crate::document::{Span, SpanRef, pending_iter};
use crate::source::SourceError;

/// One range of the text, together with every span that touches it.
#[derive(Clone, Debug)]
pub struct Passage {
    _private: (),
}

impl Passage {
    pub fn range(&self) -> Range<u32> {
        todo!()
    }

    pub fn text(&self) -> &str {
        todo!()
    }

    pub fn spans(&self) -> impl Iterator<Item = &PassageSpan> {
        pending_iter()
    }

    pub fn spans_in(&self, _layer: &str) -> impl Iterator<Item = &PassageSpan> {
        pending_iter()
    }

    pub fn error(&self, _layer: &str) -> Option<&SourceError> {
        todo!()
    }
}

/// A span as seen from a passage.
#[derive(Clone, Debug)]
pub struct PassageSpan {
    _private: (),
}

impl PassageSpan {
    pub fn layer(&self) -> &str {
        todo!()
    }

    pub fn local(&self) -> &[Range<u32>] {
        todo!()
    }

    pub fn text(&self) -> Vec<&str> {
        todo!()
    }

    pub fn starts_inside(&self) -> bool {
        todo!()
    }

    pub fn ends_inside(&self) -> bool {
        todo!()
    }

    pub fn to_ref(&self) -> SpanRef {
        todo!()
    }
}

impl Deref for PassageSpan {
    type Target = Span;

    fn deref(&self) -> &Span {
        todo!()
    }
}
