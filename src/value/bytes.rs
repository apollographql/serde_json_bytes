use crate::{bytestring::ByteString, map::Map, value::Value};
use bytes::Bytes;
use serde::de::SeqAccess;
use serde::de::{DeserializeSeed, Deserializer, MapAccess, Visitor};
use serde_json::Number;
use std::fmt;
use std::marker::PhantomData;

impl Value {
    pub fn from_bytes(data: Bytes) -> Result<Value, serde_json::Error> {
        let seed = BytesSeed::new(data.clone());
        let mut de = serde_json::Deserializer::from_slice(&data);
        seed.deserialize(&mut de)
    }
}

#[derive(Clone)]
struct BytesSeed {
    bytes: Bytes,
}

impl BytesSeed {
    fn new(bytes: Bytes) -> Self {
        BytesSeed { bytes }
    }
}

impl<'de> DeserializeSeed<'de> for BytesSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for BytesSeed {
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
        Ok(Value::String(ByteString::new(&self.bytes, value)))
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

        while let Some(elem) = match visitor.next_element_seed(self.clone()) {
            Ok(v) => v,
            Err(e) => return Err(e),
        } {
            vec.push(elem);
        }

        Ok(Value::Array(vec))
    }

    fn visit_map<V>(self, mut visitor: V) -> Result<Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        match visitor.next_key()? {
            Some(first_key) => {
                let mut values = Map::new();

                values.insert(first_key, tri!(visitor.next_value_seed(self.clone())));
                while let Some((key, value)) =
                    tri!(visitor.next_entry_seed(PhantomData, self.clone()))
                {
                    values.insert(key, value);
                }

                Ok(Value::Object(values))
            }
            None => Ok(Value::Object(Map::new())),
        }
    }
}
