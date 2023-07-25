use heck::ToSnekCase;
use kdl::{KdlDocument, KdlEntry, KdlNode};
use serde::de::{self, Error, IntoDeserializer, Unexpected};

use crate::{literal::KdlAnnotatedValueDeser, DeError, KdlAnnotatedValueWrap};

/// Deserializer for a node
#[derive(Debug, Clone)]
pub struct KdlNodeDeser<'de> {
  #[allow(dead_code)]
  name: &'de str,
  entries: &'de [KdlEntry],
  children: Option<&'de KdlDocument>,

  forwarding_to_map_from_struct: bool,
}

impl<'de> KdlNodeDeser<'de> {
  pub fn new(wrapped: &'de KdlNode) -> Self {
    Self {
      name: wrapped.name().value(),
      entries: wrapped.entries(),
      children: wrapped.children(),

      forwarding_to_map_from_struct: false,
    }
  }

  fn collect_args_props(
    &self,
  ) -> (
    Vec<KdlAnnotatedValueWrap<'de>>,
    Vec<(&'de str, KdlAnnotatedValueWrap<'de>)>,
  ) {
    let mut args = Vec::new();
    let mut props = Vec::new();
    for entry in self.entries {
      if let Some(name) = entry.name() {
        let kavr = KdlAnnotatedValueWrap::from_entry(entry);
        props.push((name.value(), kavr));
      } else {
        let kavr = KdlAnnotatedValueWrap::from_entry(entry);
        args.push(kavr);
      }
    }
    (args, props)
  }
}

macro_rules! single_scalar {
    (@ $ty:ident) => {
        paste::paste! {
            fn [< deserialize_ $ty >]<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: de::Visitor<'de>,
            {
                if let ([ref entry @ KdlEntry { .. }], true) = (self.entries, self.children.is_none()) {
                    if entry.name().is_none() {
                        // then it is actually an arg, not a prop
                        return KdlAnnotatedValueDeser::new(entry).[< deserialize_ $ty >](visitor);
                    }
                }

                Err(DeError::invalid_type(
                    Unexpected::Other(concat!(
                        "node that isn't exactly one argument deserializable as ",
                        stringify!($ty),
                        " and nothing else",
                    )),
                    &visitor,
                ))
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
    if let ([ref entry @ KdlEntry { .. }], true) =
      (self.entries, self.children.is_none())
    {
      if entry.name().is_none() {
        // then it is actually an arg
        return KdlAnnotatedValueDeser::new(entry)
          .deserialize_enum(name, variants, visitor);
      }
    }
    Err(DeError::invalid_type(
            Unexpected::Other(
                "node that isn't exactly one argument/property deserializable as enum and nothing else",
            ),
            &visitor,
        ))
  }

  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
  where
    V: de::Visitor<'de>,
  {
    let kids_all_dashes = if let Some(kids) = self.children {
      kids.nodes().iter().all(|kid| kid.name().value() == "-")
    } else {
      false
    };

    let (arguments, properties) = self.collect_args_props();

    match (
      !arguments.is_empty(),
      !properties.is_empty() || self.children.is_some(),
    ) {
      (false, false) => visitor.visit_unit(),
      (true, true) => Err(DeError::invalid_type(
        Unexpected::Other(
          "node with arguments, properties/children, or neither (and not both)",
        ),
        &visitor,
      )),
      (true, false) => {
        let mut args = arguments;
        args.reverse();
        visitor.visit_seq(SeqArgsDeser(args))
      }
      _ if kids_all_dashes => {
        visitor.visit_seq(SeqDashChildrenDeser(self.children.unwrap().nodes()))
      }
      (false, true) => self.deserialize_map(visitor),
    }
  }

  fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
  where
    V: de::Visitor<'de>,
  {
    let (args, mut properties) = self.collect_args_props();

    if !args.is_empty() {
      return Err(DeError::invalid_type(
        Unexpected::Other("node with no arguments"),
        &visitor,
      ));
    }

    properties.reverse();
    visitor.visit_map(MapDeser {
      properties,
      children: self.children.map(|x| x.nodes()),
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
    let (arguments, properties) = self.collect_args_props();

    if !properties.is_empty()
      || (arguments.is_empty() && self.children.is_none())
    {
      return Err(DeError::invalid_type(
                Unexpected::Other(
                    "node invalid as sequence (needs either only args, or children all named `-`)",
                ),
                &visitor,
            ));
    }

    if let Some(kids) = self.children {
      let kids_all_dashes =
        kids.nodes().iter().all(|kid| kid.name().value() == "-");
      if !kids_all_dashes {
        return Err(DeError::invalid_type(Unexpected::Other("node invalid as sequence (needs either only args, or children all named `-`)"), &visitor));
      }
      visitor.visit_seq(SeqDashChildrenDeser(kids.nodes()))
    } else {
      let mut args = arguments;
      args.reverse();
      visitor.visit_seq(SeqArgsDeser(args))
    }
  }
  fn deserialize_tuple<V>(
    self,
    _len: usize,
    visitor: V,
  ) -> Result<V::Value, Self::Error>
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
    let (arguments, properties) = self.collect_args_props();

    if arguments.is_empty() && properties.is_empty() && self.children.is_none()
    {
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
  fn deserialize_ignored_any<V>(
    self,
    visitor: V,
  ) -> Result<V::Value, Self::Error>
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
  properties: Vec<(&'de str, KdlAnnotatedValueWrap<'de>)>,
  children: Option<&'de [KdlNode]>,
  snekify: bool,

  value: MapDeserVal<'de>,
}

enum MapDeserVal<'de> {
  None,
  Property(KdlAnnotatedValueWrap<'de>),
  Child(&'de KdlNode),
}

impl<'de> de::MapAccess<'de> for MapDeser<'de> {
  type Error = DeError;

  fn next_key_seed<K>(
    &mut self,
    seed: K,
  ) -> Result<Option<K::Value>, Self::Error>
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
      kid.name().value()
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
      MapDeserVal::Property(prop) => {
        seed.deserialize(KdlAnnotatedValueDeser(prop))
      }
      MapDeserVal::Child(kid) => seed.deserialize(KdlNodeDeser::new(kid)),
    }
  }
}

/// Sequence deserializer for a struct with only arguments,
/// Stored backwards for better popping O time
struct SeqArgsDeser<'de>(Vec<KdlAnnotatedValueWrap<'de>>);

impl<'de> de::SeqAccess<'de> for SeqArgsDeser<'de> {
  type Error = DeError;

  fn next_element_seed<T>(
    &mut self,
    seed: T,
  ) -> Result<Option<T::Value>, Self::Error>
  where
    T: de::DeserializeSeed<'de>,
  {
    if let Some(head) = self.0.pop() {
      seed.deserialize(KdlAnnotatedValueDeser(head)).map(Some)
    } else {
      Ok(None)
    }
  }
}

/// Sequence deserializer for a struct with only children and all of the nodes are named `-`
struct SeqDashChildrenDeser<'de>(&'de [KdlNode]);

impl<'de> de::SeqAccess<'de> for SeqDashChildrenDeser<'de> {
  type Error = DeError;

  fn next_element_seed<T>(
    &mut self,
    seed: T,
  ) -> Result<Option<T::Value>, Self::Error>
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
