// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Canonical opaque installation identifiers cannot express paths or alternate spellings.
use super::Rejection;
use serde::{Deserialize, Serialize};

macro_rules! identity {
    ($name:ident, $length:literal, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);
        impl $name {
            /// Validate the fixed-length lowercase hexadecimal representation.
            /// # Errors
            /// Returns `Rejection::Identity` for noncanonical or incorrectly sized input.
            pub fn new(value: String) -> Result<Self, Rejection> {
                Self::validate(&value)?;
                Ok(Self(value))
            }
            /// Validate borrowed canonical text without allocating an identity or native authority.
            /// # Errors
            /// Returns `Rejection::Identity` for noncanonical or incorrectly sized input.
            pub fn validate(value: &str) -> Result<(), Rejection> {
                if value.len() != $length
                    || !value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(Rejection::Identity);
                }
                Ok(())
            }
            /// Return retained identifier storage for bounded host admission.
            pub fn allocated_bytes(&self) -> usize {
                self.0.capacity()
            }
            /// Borrow canonical identity text; this is not filesystem authority.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl TryFrom<String> for $name {
            type Error = Rejection;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}
identity!(InstallationId, 32, "Stable installation identity allocated by the storage adapter.");
identity!(
    GenerationId,
    32,
    "Unique immutable generation identity allocated by the storage adapter."
);
identity!(
    PackageDigest,
    64,
    "SHA-256 of the authenticated package manifest, in lowercase hexadecimal."
);
