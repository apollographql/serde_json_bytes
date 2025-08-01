use crate::{bytestring::ByteString, map::Map, value::Value};
use bytes::Bytes;
use serde::de::SeqAccess;
use serde::de::{DeserializeSeed, Deserializer, MapAccess, Visitor};
use serde_json::Number;
use std::fmt;

impl Value {
    pub fn from_bytes(data: Bytes) -> Result<Value, serde_json::Error> {
        let seed = BytesSeed::new(&data);
        let mut de = serde_json::Deserializer::from_slice(&data);
        seed.deserialize(&mut de)
    }

    pub fn to_bytes(&self) -> Bytes {
        self.to_string().into()
    }
}

/// The [`DeserializeSeed`] implementation that returns [`Value`].
///
/// Can be used in `DeserializeSeed` implementations for types that contain `Value`.
#[derive(Clone, Copy)]
pub struct BytesSeed<'data> {
    bytes: &'data Bytes,
}

impl<'data> BytesSeed<'data> {
    pub const fn new(bytes: &'data Bytes) -> Self {
        BytesSeed { bytes }
    }
}

impl<'de, 'data> DeserializeSeed<'de> for BytesSeed<'data> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de, 'data> Visitor<'de> for BytesSeed<'data> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any valid JSON value")
    }

    #[inline]
    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    #[inline]
    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    #[inline]
    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    #[inline]
    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        Ok(Number::from_f64(value).map_or(Value::Null, Value::Number))
    }

    #[inline]
    fn visit_str<E>(self, value: &str) -> Result<Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::String(value.into()))
    }

    #[inline]
    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::String(ByteString::new(self.bytes, value)))
    }

    #[inline]
    fn visit_string<E>(self, value: String) -> Result<Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::String(value.into()))
    }

    #[inline]
    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    #[inline]
    fn visit_some<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        DeserializeSeed::deserialize(self, deserializer)
    }

    #[inline]
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    #[inline]
    fn visit_seq<V>(self, mut visitor: V) -> Result<Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut vec = Vec::new();

        while let Some(elem) = visitor.next_element_seed(self)? {
            vec.push(elem);
        }

        Ok(Value::Array(vec))
    }

    fn visit_map<V>(self, mut visitor: V) -> Result<Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        match visitor.next_key_seed(ByteStringSeed::new(self.bytes))? {
            Some(first_key) => {
                let mut values = Map::new();

                values.insert(first_key, tri!(visitor.next_value_seed(self)));
                while let Some((key, value)) =
                    tri!(visitor.next_entry_seed(ByteStringSeed::new(self.bytes), self))
                {
                    values.insert(key, value);
                }

                Ok(Value::Object(values))
            }
            None => Ok(Value::Object(Map::new())),
        }
    }
}

/// A [`DeserializeSeed`] implementation that returns [`ByteString`].
///
/// Can be used in `DeserializeSeed` implementations for types that contain `ByteString`.
#[derive(Clone, Copy)]
pub struct ByteStringSeed<'data> {
    bytes: &'data Bytes,
}

impl<'data> ByteStringSeed<'data> {
    pub const fn new(bytes: &'data Bytes) -> Self {
        ByteStringSeed { bytes }
    }
}

impl<'de, 'data> DeserializeSeed<'de> for ByteStringSeed<'data> {
    type Value = ByteString;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(self)
    }
}

impl<'de, 'data> Visitor<'de> for ByteStringSeed<'data> {
    type Value = ByteString;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("any valid JSON value")
    }

    #[inline]
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(value.into())
    }

    #[inline]
    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ByteString::new(self.bytes, value))
    }

    #[inline]
    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(value.into())
    }
}
