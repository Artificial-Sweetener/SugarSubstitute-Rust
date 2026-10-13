// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse the closed incident schemas with genuine serde JSON and bounded rejected diagnostics.
//!
//! Self-describing scalar/container dispatch reaches the chosen typed visitor before
//! textual type-error formatting. Container shape remains exact; rejected container
//! positions follow genuine container cleanup: empty/whitespace closing delimiters
//! may advance further than a nonempty rejected container. Numeric syntax, EOF, domain conversion
//! and parser errors retain their original causes. The closed records use at most 64-bit
//! numbers; specialized f32/i128/u128 dispatch remains the original serde operation.
mod access;
mod visitor;
use serde::de::{DeserializeSeed, Expected, Unexpected};
use serde::{Deserialize, Deserializer};
use std::fmt;

/// Decode one complete record, retaining borrowed input and the genuine JSON error position.
/// Rejected field/tag names use fixed diagnostic categories; domain conversion is separate.
/// This private boundary's diagnostic bounds apply to the registered closed incident types.
pub(super) fn parse<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> Result<T, serde_json::Error> {
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    let value = T::deserialize(Bounded(&mut parser)).map_err(|error| error.0)?;
    parser.end()?;
    Ok(value)
}
/// Decode the typed second store-record pass and enforce the genuine parser end boundary.
/// The caller has already dropped the first pass and its parser scratch.
pub(super) fn parse_seed<'de, S: DeserializeSeed<'de>>(
    bytes: &'de [u8],
    seed: S,
) -> Result<S::Value, serde_json::Error> {
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    let value = seed.deserialize(Bounded(&mut parser)).map_err(|error| error.0)?;
    parser.end()?;
    Ok(value)
}
/// Intercept only rejected identifiers and excessive textual type/value diagnostics.
/// Nested real serde access errors retain their exact source until parser position fixup.
pub(super) struct TypedError<E = serde_json::Error>(E);
impl<E: fmt::Display> fmt::Display for TypedError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<E: fmt::Debug> fmt::Debug for TypedError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<E: std::error::Error> std::error::Error for TypedError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}
impl<E: serde::de::Error> serde::de::Error for TypedError<E> {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self(E::custom(message))
    }
    fn unknown_field(_: &str, _: &'static [&'static str]) -> Self {
        Self(E::custom("Unknown incident field"))
    }
    fn unknown_variant(_: &str, _: &'static [&'static str]) -> Self {
        Self(E::custom("Unknown incident variant"))
    }
    fn invalid_type(unexpected: Unexpected<'_>, expected: &dyn Expected) -> Self {
        if matches!(unexpected,Unexpected::Str(text) if text.len()>512) {
            Self(E::custom("Invalid incident text type"))
        } else {
            Self(E::invalid_type(unexpected, expected))
        }
    }
    fn invalid_value(unexpected: Unexpected<'_>, expected: &dyn Expected) -> Self {
        if matches!(unexpected,Unexpected::Str(text) if text.len()>512) {
            Self(E::custom("Invalid incident text value"))
        } else {
            Self(E::invalid_value(unexpected, expected))
        }
    }
    fn invalid_length(length: usize, expected: &dyn Expected) -> Self {
        Self(E::invalid_length(length, expected))
    }
    fn missing_field(field: &'static str) -> Self {
        Self(E::missing_field(field))
    }
    fn duplicate_field(field: &'static str) -> Self {
        Self(E::duplicate_field(field))
    }
}
/// Supply typed diagnostic errors over a genuine deserializer and its original owned access.
pub(super) struct Bounded<D>(pub(super) D);
struct Seed<S>(S);
impl<'de, S: DeserializeSeed<'de>> DeserializeSeed<'de> for Seed<S> {
    type Value = S::Value;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        self.0.deserialize(Bounded(deserializer)).map_err(|error| error.0)
    }
}
impl<'de, D: Deserializer<'de>> Deserializer<'de> for Bounded<D> {
    type Error = TypedError<D::Error>;
    fn deserialize_any<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_bool<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_i8<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_i16<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_i32<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_i64<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_i128<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_i128(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_u8<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_u16<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_u32<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_u64<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_u128<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_u128(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_f32<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_f32(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_f64<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_char<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_str<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_str(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_string<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_string(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_bytes<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_bytes(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_byte_buf<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_byte_buf(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_option<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_option(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_unit<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_unit_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_newtype_struct<V: serde::de::Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_newtype_struct(name, visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_seq<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::seq(visitor)).map_err(TypedError)
    }
    fn deserialize_tuple<V: serde::de::Visitor<'de>>(
        self,
        _length: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::seq(visitor)).map_err(TypedError)
    }
    fn deserialize_tuple_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        _length: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::seq(visitor)).map_err(TypedError)
    }
    fn deserialize_map<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::map(visitor)).map_err(TypedError)
    }
    fn deserialize_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_enum<V: serde::de::Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_enum(name, variants, visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_identifier<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_identifier(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn deserialize_ignored_any<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.deserialize_ignored_any(visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}
