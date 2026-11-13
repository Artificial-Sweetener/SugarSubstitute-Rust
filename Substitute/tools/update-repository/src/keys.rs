// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Ed25519 role keys, encoded as the 64 hexadecimal digits of their 32-byte seed.
//!
//! Keys are built the same way the application's test fixtures build theirs, so key identities
//! never depend on optional key-identity hash settings. Keys never live in the repository:
//! CI receives them as secrets and passes them through the environment.
use crate::{Error, invalid};
use ring::{
    rand::{SecureRandom, SystemRandom},
    signature::{Ed25519KeyPair, KeyPair},
};
use tuf::crypto::{Ed25519PrivateKey, PrivateKey, PublicKey};

const SEED_BYTES: usize = 32;

/// One role's private signing key.
pub struct SigningKey {
    key: Ed25519PrivateKey,
}
impl SigningKey {
    /// Decode a key from its hexadecimal seed; surrounding whitespace is ignored.
    /// # Errors
    /// Rejects anything but exactly 64 hexadecimal digits.
    pub fn from_hex(text: &str) -> Result<Self, Error> {
        let text = text.trim();
        if text.len() != SEED_BYTES * 2 {
            return Err(invalid("A signing key is the 64 hexadecimal digits of an Ed25519 seed"));
        }
        let mut seed = [0; SEED_BYTES];
        for (index, byte) in seed.iter_mut().enumerate() {
            *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2).unwrap_or(""), 16)
                .map_err(|_| invalid("A signing key may contain only hexadecimal digits"))?;
        }
        Self::from_seed(&seed)
    }

    /// Build a key from its 32-byte seed.
    /// # Errors
    /// Rejects a seed the Ed25519 implementation refuses.
    pub fn from_seed(seed: &[u8; SEED_BYTES]) -> Result<Self, Error> {
        let pair = Ed25519KeyPair::from_seed_unchecked(seed)
            .map_err(|_| invalid("The Ed25519 seed was refused"))?;
        let bytes = [&seed[..], pair.public_key().as_ref()].concat();
        Ok(Self { key: Ed25519PrivateKey::from_ed25519(&bytes)? })
    }

    /// Generate a new key from the operating system's secure random source.
    /// Returns the key and its hexadecimal seed, which the caller must store as a secret.
    /// # Errors
    /// Fails when the random source is unavailable.
    pub fn generate() -> Result<(Self, String), Error> {
        let mut seed = [0; SEED_BYTES];
        SystemRandom::new()
            .fill(&mut seed)
            .map_err(|_| invalid("The secure random source is unavailable"))?;
        let hex = seed.iter().map(|byte| format!("{byte:02x}")).collect();
        Ok((Self::from_seed(&seed)?, hex))
    }

    /// The public key installations trust for this role.
    pub fn public(&self) -> &PublicKey {
        self.key.public()
    }

    /// The key as the TUF signer.
    pub(crate) fn signer(&self) -> &dyn PrivateKey {
        &self.key
    }
}
