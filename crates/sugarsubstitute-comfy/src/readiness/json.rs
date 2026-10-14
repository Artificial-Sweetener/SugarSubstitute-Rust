// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate duplicate keys and bounded collection nesting before typed conversion.
use super::{MAX_EXTENSION_PATHS, ReadinessError};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

use std::fmt;
const MAX_DEPTH: u8 = 32;
const MAX_OBJECT_FIELDS: usize = 128;
struct Seed(u8);
impl<'de> DeserializeSeed<'de> for Seed {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        if self.0 > MAX_DEPTH {
            return Err(de::Error::custom("JSON nesting bound"));
        }
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<(), E> {
        if v.is_finite() { Ok(()) } else { Err(E::custom("Nonfinite JSON number")) }
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_string<E: de::Error>(self, _: String) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let mut count = 0;
        while a.next_element_seed(Seed(self.0 + 1))?.is_some() {
            if count == MAX_EXTENSION_PATHS {
                return Err(de::Error::custom("JSON array bound"));
            }
            count += 1;
        }
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let mut keys: [Option<String>; MAX_OBJECT_FIELDS] = std::array::from_fn(|_| None);
        let mut count = 0;
        while let Some(key) = a.next_key::<String>()? {
            if count == MAX_OBJECT_FIELDS
                || keys[..count].iter().any(|existing| existing.as_ref() == Some(&key))
                || key == "schema_version"
            {
                return Err(de::Error::custom("Duplicate, excessive or versioned JSON fields"));
            }
            keys[count] = Some(key);
            count += 1;
            a.next_value_seed(Seed(self.0 + 1))?;
        }
        Ok(())
    }
}
pub(super) fn decode(bytes: &[u8], limit: usize) -> Result<(), ReadinessError> {
    if bytes.len() > limit {
        return Err(ReadinessError::TooLarge);
    }
    let mut d = serde_json::Deserializer::from_slice(bytes);
    Seed(0).deserialize(&mut d).map_err(ReadinessError::Json)?;
    d.end().map_err(ReadinessError::Json)?;
    Ok(())
}
