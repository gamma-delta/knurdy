use crate::{DeError, KdlAnnotatedLiteral, KdlLiteral};

use std::convert::TryInto;

use serde::de::{self, Error, IntoDeserializer, Unexpected, Visitor};

macro_rules! passthru_to_literal {
    (@ $ty:ident) => {
        paste::paste! {
            fn [< deserialize_ $ty >]<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>,
            {
                KdlLiteralDeser(self.literal).[< deserialize_ $ty >](visitor)
            }
        }
    };
    ( $($ty:ident)* ) => {
        $(
            passthru_to_literal!(@ $ty);
        )*
    };
}
macro_rules! deser_int_literal {
    (@ $ty:ty) => {
        paste::paste! {
            fn [< deserialize_ $ty >]<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>,
            {
                match self.0 {
                    KdlLiteral::Int(it) => {
                        let squished: $ty = (*it).try_into()?;
                        visitor.[< visit_ $ty >](squished)
                    }
                    oh_no => Err(DeError::invalid_type(oh_no.unexpected(), &visitor)),
                }
            }
        }
    };
    ( $($ty:ty)* ) => {
        $(
            deser_int_literal!(@ $ty);
        )*
    };
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct KdlAnnotatedLiteralDeser<'de> {
    annotation: Option<&'de str>,
    literal: &'de KdlLiteral,
}

impl<'de> KdlAnnotatedLiteralDeser<'de> {
    pub(crate) fn new(wrapped: &'de KdlAnnotatedLiteral) -> Self {
        Self {
            annotation: wrapped.annotation.as_ref().map(|s| s.as_str()),
            literal: &wrapped.literal,
        }
    }

    fn annotation_is(&self, s: &str) -> bool {
        match self.annotation {
            Some(it) => it == s,
            None => false,
        }
    }
}

struct KdlLiteralDeser<'de>(&'de KdlLiteral);

impl<'de> de::Deserializer<'de> for KdlAnnotatedLiteralDeser<'de> {
    type Error = DeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.literal {
            KdlLiteral::String(_) => self.deserialize_str(visitor),
            KdlLiteral::Int(_) => self.deserialize_i64(visitor),
            KdlLiteral::Float(_) => self.deserialize_f64(visitor),
            KdlLiteral::Bool(_) => self.deserialize_bool(visitor),
            KdlLiteral::Null => self.deserialize_unit(visitor),
        }
    }

    passthru_to_literal! {
        u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 bool
        str string identifier unit seq map ignored_any
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.literal {
            KdlLiteral::String(s) if self.annotation_is("byte") => match s.as_bytes() {
                [b] => visitor.visit_u8(*b),
                _ => Err(DeError::ByteAnnotationLen),
            },
            other => KdlLiteralDeser(other).deserialize_u8(visitor),
        }
    }
    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.literal {
            KdlLiteral::String(s) if self.annotation_is("char") => {
                let mut chars = s.chars();
                let ch0 = chars.next();
                let ch1 = chars.next();
                match (ch0, ch1) {
                    (Some(ch0), None) => visitor.visit_char(ch0),
                    _ => Err(DeError::CharAnnotationLen),
                }
            }
            other => KdlLiteralDeser(other).deserialize_u8(visitor),
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.literal {
            KdlLiteral::String(s) => {
                if self.annotation_is("base64") {
                    let b64 = base64::decode(s.as_str())?;
                    visitor.visit_byte_buf(b64)
                } else {
                    visitor.visit_bytes(s.as_bytes())
                }
            }
            oh_no => Err(DeError::invalid_type(oh_no.unexpected(), &visitor)),
        }
    }
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.literal {
            KdlLiteral::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    // Unit enums are parsed as string variants.
    // Non-unit enums are parsed with the annotation as the variant.
    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let (variant, value) = match self.annotation {
            // Unit variant
            None => match &self.literal {
                KdlLiteral::String(s) => (s.as_str(), None),
                ohno => return Err(DeError::invalid_type(ohno.unexpected(), &visitor)),
            },
            Some(ann) => (ann, Some(self.literal)),
        };
        visitor.visit_enum(EnumLiteralDeserializer { variant, value })
    }

    // other passthrus that i can't do with the easy macro
    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }
    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }
}

impl<'de> de::Deserializer<'de> for KdlLiteralDeser<'de> {
    type Error = DeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.0 {
            KdlLiteral::String(_) => self.deserialize_str(visitor),
            KdlLiteral::Int(_) => self.deserialize_i64(visitor),
            KdlLiteral::Float(_) => self.deserialize_f64(visitor),
            KdlLiteral::Bool(_) => self.deserialize_bool(visitor),
            KdlLiteral::Null => self.deserialize_unit(visitor),
        }
    }

    deser_int_literal! {
        u8 u16 u32 u64 u128 i8 i16 i32 i64 i128
    }
    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            KdlLiteral::Int(it) => {
                let squished: u32 = (*it).try_into()?;
                let squished_again: char = squished.try_into()?;
                visitor.visit_char(squished_again)
            }
            oh_no => Err(DeError::invalid_type(oh_no.unexpected(), &visitor)),
        }
    }
    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            KdlLiteral::Float(f) => {
                // For some reason there doesn't seem to be Into or TryInto impls for f64 => f32?
                visitor.visit_f32(*f as f32)
            }
            oh_no => Err(DeError::invalid_type(oh_no.unexpected(), &visitor)),
        }
    }
    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            KdlLiteral::Float(f) => visitor.visit_f64(*f),
            oh_no => Err(DeError::invalid_type(oh_no.unexpected(), &visitor)),
        }
    }
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            KdlLiteral::Bool(b) => visitor.visit_bool(*b),
            oh_no => Err(DeError::invalid_type(oh_no.unexpected(), &visitor)),
        }
    }

    // byte stuff

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            KdlLiteral::String(s) => visitor.visit_str(s.as_str()),
            oh_no => Err(DeError::invalid_type(oh_no.unexpected(), &visitor)),
        }
    }
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            KdlLiteral::String(s) => visitor.visit_bytes(s.as_bytes()),
            oh_no => Err(DeError::invalid_type(oh_no.unexpected(), &visitor)),
        }
    }
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    // Units must be null; why are you doing this?
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            KdlLiteral::Null => visitor.visit_unit(),
            oh_no => Err(DeError::invalid_type(oh_no.unexpected(), &visitor)),
        }
    }
    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    // Passthru to whatever
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            KdlLiteral::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    // This should never be called
    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unreachable!("should never pass through to here")
    }

    // Literals can't be sequences so all of these forward to error
    #[inline]
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(DeError::invalid_type(self.0.unexpected(), &visitor))
    }
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }
    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // and they also can't be maps
    #[inline]
    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

/// Deserializes either `"variant"` into a unit enum, or `(variant)"value"` into a newtype enum (in an argument/property context)
struct EnumLiteralDeserializer<'a> {
    variant: &'a str,
    value: Option<&'a KdlLiteral>,
}

impl<'de> de::EnumAccess<'de> for EnumLiteralDeserializer<'de> {
    type Error = DeError;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let variant = self.variant.into_deserializer();
        seed.deserialize(variant).map(|v| (v, self))
    }
}

impl<'de> de::VariantAccess<'de> for EnumLiteralDeserializer<'de> {
    type Error = DeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        match self.value {
            None => Ok(()),
            // this means we went `(variant)"some extant data"`
            Some(value) => Err(DeError::invalid_type(
                value.unexpected(),
                &"unannotated string",
            )),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.value {
            // Deserialize the newtype data
            Some(value) => seed.deserialize(KdlLiteralDeser(value)),
            None => Err(DeError::invalid_type(
                Unexpected::UnitVariant,
                &"annotated literal",
            )),
        }
    }

    // This is never valid for literals
    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(DeError::invalid_type(
            Unexpected::Other("argument/property"),
            &visitor,
        ))
    }
    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(DeError::invalid_type(
            Unexpected::Other("argument/property"),
            &visitor,
        ))
    }
}
