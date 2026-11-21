use alloc::string::String;

use crate::document::{IntoValues, Span, Value, pending_iter};
use crate::source::SpanSource;

#[derive(Debug)]
pub struct Layer {
    _private: (),
}

impl Layer {
    pub fn new(_id: impl Into<String>) -> Self {
        todo!()
    }

    pub fn source(_id: impl Into<String>, _source: impl SpanSource + 'static) -> Self {
        todo!()
    }

    pub fn with_name(self, _name: impl Into<String>) -> Self {
        todo!()
    }

    pub fn with_attr(self, _key: impl Into<String>, _values: impl IntoValues) -> Self {
        todo!()
    }

    pub fn attrs(&self) -> impl Iterator<Item = (&str, &Value)> {
        pending_iter()
    }

    pub fn id(&self) -> &str {
        todo!()
    }

    pub fn name(&self) -> &str {
        todo!()
    }

    pub fn is_source(&self) -> bool {
        todo!()
    }

    pub fn spans(&self) -> impl Iterator<Item = &Span> {
        pending_iter()
    }
}

impl From<&str> for Layer {
    fn from(_id: &str) -> Self {
        todo!()
    }
}
