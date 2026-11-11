// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode only the served URL array after complete bounded syntax validation.
use super::{MAX_EXTENSION_PATHS, ReadinessError};
use serde::de::{self, SeqAccess, Visitor};
use std::fmt;
struct Paths;
impl<'de> Visitor<'de> for Paths {
    type Value = Vec<String>;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an extension URL array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        // Syntax already bounded every array; fixed exact headers avoid geometric resize overlap.
        let mut paths = Vec::with_capacity(MAX_EXTENSION_PATHS);
        while let Some(path) = access.next_element::<String>()? {
            if paths.len() == MAX_EXTENSION_PATHS {
                return Err(de::Error::custom("Extension URL array bound"));
            }
            paths.push(path);
        }
        Ok(paths)
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<Self::Value, E> {
        Err(E::custom("Expected extension URL array"))
    }
    fn visit_string<E: de::Error>(self, _: String) -> Result<Self::Value, E> {
        Err(E::custom("Expected extension URL array"))
    }
}
pub(super) fn decode(bytes: &[u8]) -> Result<Vec<String>, ReadinessError> {
    use serde::Deserializer;
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let paths = decoder.deserialize_any(Paths).map_err(ReadinessError::Json)?;
    decoder.end().map_err(ReadinessError::Json)?;
    Ok(paths)
}
