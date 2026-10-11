// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate duplicate keys and bounded collection nesting before typed conversion.
use super::{MAX_EXTENSION_PATHS, ReadinessError};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;
const MAX_DEPTH: u8 = 32;
const MAX_OBJECT_FIELDS: usize = 128;
struct Seed(u8);
impl<'de> DeserializeSeed<'de> for Seed {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        if self.0 > MAX_DEPTH {
            return Err(de::Error::custom("JSON nesting bound"));
        }
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Number::from_f64(v).map(Value::Number).ok_or_else(|| E::custom("Nonfinite JSON number"))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.into()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(v) = a.next_element_seed(Seed(self.0 + 1))? {
            if values.len() == MAX_EXTENSION_PATHS {
                return Err(de::Error::custom("JSON array bound"));
            }
            values.push(v);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = a.next_key::<String>()? {
            if values.len() == MAX_OBJECT_FIELDS
                || values.contains_key(&key)
                || key == "schema_version"
            {
                return Err(de::Error::custom("Duplicate, excessive or versioned JSON fields"));
            }
            let value = a.next_value_seed(Seed(self.0 + 1))?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
pub(super) fn decode(bytes: &[u8], limit: usize) -> Result<Value, ReadinessError> {
    if bytes.len() > limit {
        return Err(ReadinessError::TooLarge);
    }
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let value = Seed(0).deserialize(&mut d).map_err(ReadinessError::Json)?;
    d.end().map_err(ReadinessError::Json)?;
    Ok(value)
}
