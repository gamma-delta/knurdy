/*!
Extremely opinionated crate for an intermediate representation between [`kdl`](https://crates.io/crates/kdl) and serde.
*/

mod literal;
mod node;

use std::{char::CharTryFromError, convert::Infallible, num::TryFromIntError};

use ahash::AHashMap;
use node::KdlNodeDeser;
use smol_str::SmolStr;

use serde::de::{self, Unexpected};
use thiserror::Error;

use knuffel::{traits::ErrorSpan, Decode, DecodeScalar};

/// An node featuring the arguments, properties, and children, and a name.
#[derive(Debug, Clone, PartialEq)]
pub struct KdlNode {
    pub name: SmolStr,
    pub arguments: Vec<KdlAnnotatedLiteral>,
    pub properties: AHashMap<SmolStr, KdlAnnotatedLiteral>,
    pub children: Option<Vec<KdlNode>>,
}

impl<S: ErrorSpan> Decode<S> for KdlNode {
    fn decode_node(
        node: &knuffel::ast::SpannedNode<S>,
        ctx: &mut knuffel::decode::Context<S>,
    ) -> Result<Self, knuffel::errors::DecodeError<S>> {
        let name = SmolStr::from(&*node.node_name);
        let arguments = node
            .arguments
            .iter()
            .map(|arg| KdlAnnotatedLiteral::decode(arg, ctx))
            .collect::<Result<_, _>>()?;
        let properties = node
            .properties
            .iter()
            .map(|(k, v)| {
                let v = KdlAnnotatedLiteral::decode(v, ctx)?;
                Ok((k.into(), v))
            })
            .collect::<Result<_, _>>()?;
        let children = match &node.children {
            None => None,
            Some(kids) => {
                let kids = kids
                    .iter()
                    .map(|kid| KdlNode::decode_node(kid, ctx))
                    .collect::<Result<_, _>>()?;
                Some(kids)
            }
        };

        Ok(Self {
            name,
            arguments,
            properties,
            children,
        })
    }
}

/// Raw literal: argument or property
#[derive(Debug, Clone, PartialEq)]
pub enum KdlLiteral {
    String(SmolStr),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

impl KdlLiteral {
    fn unexpected(&self) -> Unexpected {
        match self {
            KdlLiteral::String(s) => Unexpected::Str(s.as_str()),
            KdlLiteral::Int(_) => Unexpected::Other("int"),
            KdlLiteral::Float(f) => Unexpected::Float(*f),
            KdlLiteral::Bool(b) => Unexpected::Bool(*b),
            KdlLiteral::Null => Unexpected::Unit,
        }
    }
}

/// A literal and possible annotation
#[derive(Debug, Clone, PartialEq)]
pub struct KdlAnnotatedLiteral {
    pub annotation: Option<SmolStr>,
    pub literal: KdlLiteral,
}

impl KdlAnnotatedLiteral {
    pub fn new(annotation: Option<SmolStr>, literal: KdlLiteral) -> Self {
        Self {
            annotation,
            literal,
        }
    }
}

impl<S: ErrorSpan> DecodeScalar<S> for KdlAnnotatedLiteral {
    fn type_check(
        _type_name: &Option<knuffel::span::Spanned<knuffel::ast::TypeName, S>>,
        _ctx: &mut knuffel::decode::Context<S>,
    ) {
        // no-op
    }

    // thIs ShouLd nOt Be oVerWritTen
    fn decode(
        value: &knuffel::ast::Value<S>,
        ctx: &mut knuffel::decode::Context<S>,
    ) -> Result<Self, knuffel::errors::DecodeError<S>> {
        let literal = match &*value.literal {
            knuffel::ast::Literal::Null => KdlLiteral::Null,
            knuffel::ast::Literal::Bool(b) => KdlLiteral::Bool(*b),
            knuffel::ast::Literal::String(s) => KdlLiteral::String(SmolStr::from(&**s)),
            knuffel::ast::Literal::Int(_) => {
                let conv = DecodeScalar::decode(value, ctx)?;
                KdlLiteral::Int(conv)
            }
            knuffel::ast::Literal::Decimal(_) => {
                let conv = DecodeScalar::decode(value, ctx)?;
                KdlLiteral::Float(conv)
            }
        };
        let annotation = value
            .type_name
            .as_ref()
            .map(|ann| SmolStr::new(ann.as_str()));
        Ok(KdlAnnotatedLiteral {
            annotation,
            literal,
        })
    }

    fn raw_decode(
        _value: &knuffel::span::Spanned<knuffel::ast::Literal, S>,
        _ctx: &mut knuffel::decode::Context<S>,
    ) -> Result<Self, knuffel::errors::DecodeError<S>> {
        panic!("shouldn't call this directly, only with type name")
    }
}

impl<'de> de::IntoDeserializer<'de, DeError> for &'de KdlNode {
    type Deserializer = KdlNodeDeser<'de>;

    fn into_deserializer(self) -> Self::Deserializer {
        KdlNodeDeser::new(self)
    }
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
