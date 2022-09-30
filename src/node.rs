use ahash::AHashMap;
use heck::ToSnekCase;
use serde::de::{self, Error, IntoDeserializer, Unexpected};
use smol_str::SmolStr;

use crate::{literal::KdlAnnotatedLiteralDeser, DeError, KdlAnnotatedLiteral, KdlNode};

/// Deserializer for a node
#[derive(Debug, Clone)]
pub struct KdlNodeDeser<'de> {
    #[allow(dead_code)]
    name: &'de str,
    arguments: &'de [KdlAnnotatedLiteral],
    properties: &'de AHashMap<SmolStr, KdlAnnotatedLiteral>,
    children: Option<&'de [KdlNode]>,

    forwarding_to_map_from_struct: bool,
}

impl<'de> KdlNodeDeser<'de> {
    pub fn new(wrapped: &'de KdlNode) -> Self {
        Self {
            name: wrapped.name.as_str(),
            arguments: &wrapped.arguments,
            properties: &wrapped.properties,
            children: wrapped.children.as_ref().map(|kids| kids.as_slice()),
            forwarding_to_map_from_struct: false,
        }
    }
}

macro_rules! single_scalar {
    (@ $ty:ident) => {
        paste::paste! {
            fn [< deserialize_ $ty >]<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: de::Visitor<'de>,
            {
                match (
                    self.arguments,
                    self.properties.is_empty(),
                    self.children.is_none(),
                ) {
                    ([it], true, true) => KdlAnnotatedLiteralDeser::new(it).[< deserialize_ $ty >](visitor),
                    _ => Err(DeError::invalid_type(
                        Unexpected::Other(concat!(
                            "node that isn't exactly one argument deserializable as ",
                            stringify!($ty),
                            " and nothing else",
                        )),
                        &visitor,
                    )),
                }
            }
        }
    };
    ( $($ty:ident)* ) => {
        $(
            single_scalar!(@ $ty);
        )*
    };
}

impl<'de> de::Deserializer<'de> for KdlNodeDeser<'de> {
    type Error = DeError;

    single_scalar! {
        u8 u16 u32 u64 i8 i16 i32 i64 char bool f32 f64
        str string bytes byte_buf identifier
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match (
            self.arguments,
            self.properties.is_empty(),
            self.children.is_none(),
        ) {
            ([it], true, true) => {
                KdlAnnotatedLiteralDeser::new(it).deserialize_enum(name, variants, visitor)
            }
            _ => Err(DeError::invalid_type(
                Unexpected::Other(
                    "node that isn't exactly one argument deserializable as enum and nothing else",
                ),
                &visitor,
            )),
        }
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let kids_all_dashes = if let Some(kids) = self.children {
            kids.iter().all(|kid| kid.name == "-")
        } else {
            false
        };

        match (
            !self.arguments.is_empty(),
            !self.properties.is_empty() || self.children.is_some(),
        ) {
            (false, false) => visitor.visit_unit(),
            (true, true) => Err(DeError::invalid_type(
                Unexpected::Other(
                    "node with arguments, properties/children, or neither (and not both)",
                ),
                &visitor,
            )),
            (true, false) => visitor.visit_seq(SeqArgsDeser(self.arguments)),
            _ if kids_all_dashes => visitor.visit_seq(SeqDashChildrenDeser(self.children.unwrap())),
            (false, true) => self.deserialize_map(visitor),
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if !self.arguments.is_empty() {
            return Err(DeError::invalid_type(
                Unexpected::Other("node with no arguments"),
                &visitor,
            ));
        }

        let mut properties: Vec<_> = self
            .properties
            .iter()
            .map(|(key, val)| (key.as_str(), val))
            .collect();
        properties.reverse();
        visitor.visit_map(MapDeser {
            properties,
            children: self.children,
            value: MapDeserVal::None,
            snekify: self.forwarding_to_map_from_struct,
        })
    }
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let self2 = Self {
            forwarding_to_map_from_struct: true,
            ..self
        };
        self2.deserialize_map(visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if !self.properties.is_empty() || (self.arguments.is_empty() && self.children.is_none()) {
            return Err(DeError::invalid_type(
                Unexpected::Other(
                    "node invalid as sequence (needs either only args, or children all named `-`)",
                ),
                &visitor,
            ));
        }

        if let Some(kids) = self.children {
            let kids_all_dashes = kids.iter().all(|kid| kid.name == "-");
            if !kids_all_dashes {
                return Err(DeError::invalid_type(Unexpected::Other("node invalid as sequence (needs either only args, or children all named `-`)"), &visitor));
            }
            visitor.visit_seq(SeqDashChildrenDeser(kids))
        } else {
            visitor.visit_seq(SeqArgsDeser(self.arguments))
        }
    }
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
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
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if self.arguments.is_empty() && self.properties.is_empty() && self.children.is_none() {
            visitor.visit_unit()
        } else {
            Err(DeError::invalid_type(
                Unexpected::Other("node with arguments or properties or children"),
                &visitor,
            ))
        }
    }
    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_some(self)
    }
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }
}

struct MapDeser<'de> {
    /// These are in *backwards* order so it's cheap to pop the back one off
    properties: Vec<(&'de str, &'de KdlAnnotatedLiteral)>,
    children: Option<&'de [KdlNode]>,
    snekify: bool,

    value: MapDeserVal<'de>,
}

enum MapDeserVal<'de> {
    None,
    Property(&'de KdlAnnotatedLiteral),
    Child(&'de KdlNode),
}

impl<'de> de::MapAccess<'de> for MapDeser<'de> {
    type Error = DeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        if !matches!(self.value, MapDeserVal::None) {
            return Err(DeError::custom("map visitor requested two keys in a row"));
        }

        // more like *pop*erties amirite
        let key = if let Some((key, val)) = self.properties.pop() {
            self.value = MapDeserVal::Property(val);
            key
        } else if let Some([kid, tail @ ..]) = self.children {
            // lispily pop the front
            self.children = Some(tail);
            self.value = MapDeserVal::Child(kid);
            kid.name.as_str()
        } else {
            return Ok(None);
        };
        let snek = if self.snekify {
            ToSnekCase::to_snek_case(key)
        } else {
            key.to_owned()
        };
        seed.deserialize(snek.into_deserializer()).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        match std::mem::replace(&mut self.value, MapDeserVal::None) {
            MapDeserVal::None => Err(DeError::custom(
                "map visitor requested a value without a key",
            )),
            MapDeserVal::Property(prop) => seed.deserialize(KdlAnnotatedLiteralDeser::new(prop)),
            MapDeserVal::Child(kid) => seed.deserialize(KdlNodeDeser::new(kid)),
        }
    }
}

/// Sequence deserializer for a struct with only arguments
struct SeqArgsDeser<'de>(&'de [KdlAnnotatedLiteral]);

impl<'de> de::SeqAccess<'de> for SeqArgsDeser<'de> {
    type Error = DeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        if let [head, tail @ ..] = self.0 {
            self.0 = tail;
            seed.deserialize(KdlAnnotatedLiteralDeser::new(head))
                .map(Some)
        } else {
            Ok(None)
        }
    }
}

/// Sequence deserializer for a struct with only children and all of the nodes are named `-`
struct SeqDashChildrenDeser<'de>(&'de [KdlNode]);

impl<'de> de::SeqAccess<'de> for SeqDashChildrenDeser<'de> {
    type Error = DeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        if let [head, tail @ ..] = self.0 {
            self.0 = tail;
            seed.deserialize(KdlNodeDeser::new(head)).map(Some)
        } else {
            Ok(None)
        }
    }
}
