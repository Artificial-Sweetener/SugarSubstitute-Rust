// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain typed nested seeds over genuine parser access and unwrap errors at its position boundary.
use super::{Seed, TypedError, visitor};
use serde::de::{DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};
pub(super) struct Forward<A>(pub(super) A);
impl<'de, A: SeqAccess<'de>> SeqAccess<'de> for Forward<A> {
    type Error = TypedError<A::Error>;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        self.0.next_element_seed(Seed(seed)).map_err(TypedError)
    }
    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}
impl<'de, A: MapAccess<'de>> MapAccess<'de> for Forward<A> {
    type Error = TypedError<A::Error>;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        self.0.next_key_seed(Seed(seed)).map_err(TypedError)
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.next_value_seed(Seed(seed)).map_err(TypedError)
    }
    fn next_entry_seed<K: DeserializeSeed<'de>, V: DeserializeSeed<'de>>(
        &mut self,
        key: K,
        value: V,
    ) -> Result<Option<(K::Value, V::Value)>, Self::Error> {
        self.0.next_entry_seed(Seed(key), Seed(value)).map_err(TypedError)
    }
    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}
impl<'de, A: EnumAccess<'de>> EnumAccess<'de> for Forward<A> {
    type Error = TypedError<A::Error>;
    type Variant = Forward<A::Variant>;
    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error> {
        self.0
            .variant_seed(Seed(seed))
            .map(|(value, variant)| (value, Forward(variant)))
            .map_err(TypedError)
    }
}
impl<'de, A: VariantAccess<'de>> VariantAccess<'de> for Forward<A> {
    type Error = TypedError<A::Error>;
    fn unit_variant(self) -> Result<(), Self::Error> {
        self.0.unit_variant().map_err(TypedError)
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, Self::Error> {
        self.0.newtype_variant_seed(Seed(seed)).map_err(TypedError)
    }
    fn tuple_variant<V: Visitor<'de>>(
        self,
        length: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.tuple_variant(length, visitor::Forward::all(visitor)).map_err(TypedError)
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.struct_variant(fields, visitor::Forward::all(visitor)).map_err(TypedError)
    }
}
