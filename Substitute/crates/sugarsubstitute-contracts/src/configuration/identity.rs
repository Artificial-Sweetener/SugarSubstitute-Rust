// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Validate isolated profile identities and canonical product locale choices.

use super::ConfigurationError;
use crate::localization::LOCALES;

/// Select one isolated browser profile namespace without accepting path syntax.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileId(String);

impl ProfileId {
    /// Validate 1 to 64 ASCII letters, digits, underscores or hyphens.
    ///
    /// Returns [`ConfigurationError::InvalidProfile`] for all other input.
    pub fn new(value: String) -> Result<Self, ConfigurationError> {
        Self::validate(&value)?;
        Ok(Self(value))
    }

    /// Validate an exact borrowed namespace using the owned identifier grammar.
    /// # Errors
    /// Returns [`ConfigurationError::InvalidProfile`] for invalid spelling.
    pub fn validate(value: &str) -> Result<(), ConfigurationError> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(ConfigurationError::InvalidProfile);
        }
        Ok(())
    }

    /// Borrow the validated namespace identity without transforming its spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return actual retained namespace allocation capacity, in UTF-8 bytes.
    /// Admission owners use this metadata without cloning or granting path authority.
    pub fn allocated_bytes(&self) -> usize {
        self.0.capacity()
    }
}

/// An exact member of the authoritative supported-locale registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocaleId(&'static str);

impl LocaleId {
    /// Resolve an exact registry identity, rejecting unknown values without fallback.
    ///
    /// Returns [`ConfigurationError::UnsupportedLocale`] on an unknown spelling.
    pub fn new(value: &str) -> Result<Self, ConfigurationError> {
        LOCALES
            .iter()
            .find(|locale| locale.id == value)
            .map(|locale| Self(locale.id))
            .ok_or(ConfigurationError::UnsupportedLocale)
    }

    /// Borrow the canonical registry identity.
    pub fn as_str(self) -> &'static str {
        self.0
    }
}
