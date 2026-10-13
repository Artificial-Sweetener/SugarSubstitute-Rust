// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Forward the actual parser's scalar and compound observations without changing dispatch.
use super::{Bounded, TypedError, access};
use serde::de::{Deserializer, EnumAccess, MapAccess, SeqAccess, Visitor};
use std::fmt;
enum Shape {
    All,
    Map,
    Seq,
}
pub(super) struct Forward<V>(V, Shape);
impl<V> Forward<V> {
    pub(super) fn all(visitor: V) -> Self {
        Self(visitor, Shape::All)
    }
    pub(super) fn map(visitor: V) -> Self {
        Self(visitor, Shape::Map)
    }
    pub(super) fn seq(visitor: V) -> Self {
        Self(visitor, Shape::Seq)
    }
}
impl<'de, V: Visitor<'de>> Visitor<'de> for Forward<V> {
    type Value = V::Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.expecting(f)
    }
    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
        self.0.visit_bool::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_i8<E: serde::de::Error>(self, value: i8) -> Result<Self::Value, E> {
        self.0.visit_i8::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_i16<E: serde::de::Error>(self, value: i16) -> Result<Self::Value, E> {
        self.0.visit_i16::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_i32<E: serde::de::Error>(self, value: i32) -> Result<Self::Value, E> {
        self.0.visit_i32::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
        self.0.visit_i64::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_i128<E: serde::de::Error>(self, value: i128) -> Result<Self::Value, E> {
        self.0.visit_i128::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_u8<E: serde::de::Error>(self, value: u8) -> Result<Self::Value, E> {
        self.0.visit_u8::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_u16<E: serde::de::Error>(self, value: u16) -> Result<Self::Value, E> {
        self.0.visit_u16::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_u32<E: serde::de::Error>(self, value: u32) -> Result<Self::Value, E> {
        self.0.visit_u32::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
        self.0.visit_u64::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_u128<E: serde::de::Error>(self, value: u128) -> Result<Self::Value, E> {
        self.0.visit_u128::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_f32<E: serde::de::Error>(self, value: f32) -> Result<Self::Value, E> {
        self.0.visit_f32::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
        self.0.visit_f64::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_char<E: serde::de::Error>(self, value: char) -> Result<Self::Value, E> {
        self.0.visit_char::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
        self.0.visit_str::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_borrowed_str<E: serde::de::Error>(self, value: &'de str) -> Result<Self::Value, E> {
        self.0.visit_borrowed_str::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
        self.0.visit_string::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_bytes<E: serde::de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
        self.0.visit_bytes::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_borrowed_bytes<E: serde::de::Error>(self, value: &'de [u8]) -> Result<Self::Value, E> {
        self.0.visit_borrowed_bytes::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_byte_buf<E: serde::de::Error>(self, value: Vec<u8>) -> Result<Self::Value, E> {
        self.0.visit_byte_buf::<TypedError<E>>(value).map_err(|error| error.0)
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        self.0.visit_none::<TypedError<E>>().map_err(|error| error.0)
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        self.0.visit_unit::<TypedError<E>>().map_err(|error| error.0)
    }
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        self.0.visit_some(Bounded(deserializer)).map_err(|error| error.0)
    }
    fn visit_newtype_struct<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        self.0.visit_newtype_struct(Bounded(deserializer)).map_err(|error| error.0)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
        if matches!(self.1, Shape::Map) {
            return Err(<TypedError<A::Error> as serde::de::Error>::invalid_type(
                serde::de::Unexpected::Seq,
                &self,
            )
            .0);
        }
        self.0.visit_seq(access::Forward(access)).map_err(|error| error.0)
    }
    fn visit_map<A: MapAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
        if matches!(self.1, Shape::Seq) {
            return Err(<TypedError<A::Error> as serde::de::Error>::invalid_type(
                serde::de::Unexpected::Map,
                &self,
            )
            .0);
        }
        self.0.visit_map(access::Forward(access)).map_err(|error| error.0)
    }
    fn visit_enum<A: EnumAccess<'de>>(self, access: A) -> Result<Self::Value, A::Error> {
        self.0.visit_enum(access::Forward(access)).map_err(|error| error.0)
    }
}
