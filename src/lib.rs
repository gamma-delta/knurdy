#![doc = include_str!("../README.md")]

mod literal;
mod node;

pub use literal::KdlAnnotatedValueDeser;
pub use node::KdlNodeDeser;

use std::{char::CharTryFromError, convert::Infallible, num::TryFromIntError};

use kdl::{KdlEntry, KdlNode, KdlValue};
use serde::{
    de::{self},
    Deserialize,
};
use thiserror::Error;

/// Deserialize a `KdlNode`.
pub fn deserialize_node<'de, T: Deserialize<'de>>(kdl: &'de KdlNode) -> Result<T, DeError> {
    let deserializer = KdlNodeDeser::new(kdl);
    T::deserialize(deserializer)
}

#[derive(Error, Debug)]
pub enum DeError {
    #[error("the deserialize impl on the type reported an error: {0}")]
    VisitorError(String),
    #[error("tuple struct {0} requires only arguments, no properties or children")]
    TupleStructWithNotJustArgs(&'static str),
    #[error("on type {type_name}, expected {expected} fields but got {got}")]
    MismatchedTupleStructCount {
        expected: usize,
        got: usize,
        type_name: &'static str,
    },
    #[error("could not turn fit the given int into the target size: {0}")]
    IntSize(#[from] TryFromIntError),
    #[error("could not interpret the int as a char: {0}")]
    InvalidChar(#[from] CharTryFromError),
    #[error("could not decode base64: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("a string annotated with (byte) must be 1 byte long to be interpreted as a u8")]
    ByteAnnotationLen,
    #[error("a string annotated with (char) must be 1 char long to be interpreted as a char")]
    CharAnnotationLen,

    #[error("{0}")]
    MismatchedType(String),
}

impl de::Error for DeError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::VisitorError(msg.to_string())
    }
}

impl From<Infallible> for DeError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

#[derive(Debug, Clone, Copy)]
struct KdlAnnotatedValueWrap<'de> {
    annotation: Option<&'de str>,
    value: &'de KdlValue,
}

impl<'de> KdlAnnotatedValueWrap<'de> {
    // fn new(annotation: Option<&'de str>, value: &'de KdlValue) -> Self {
    //     Self { annotation, value }
    // }

    fn from_entry(entry: &'de KdlEntry) -> Self {
        Self {
            annotation: entry.ty().map(|s| s.value()),
            value: entry.value(),
        }
    }
}
