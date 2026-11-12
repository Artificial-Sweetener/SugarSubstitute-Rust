// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read the identity a pack declares in its `pyproject.toml`.
//!
//! ComfyUI Manager recognises a registry install by `pyproject.toml` beside `.tracking` and
//! reads the pack id and version from its `[project]` table.

use super::error::{ErrorCode, PackInstallError, Result};
use std::fmt;

/// The largest `pyproject.toml` read.
pub const MAX_PYPROJECT_BYTES: usize = 256 * 1024;

/// A plain `major.minor.patch` release version, ordered numerically.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReleaseVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl ReleaseVersion {
    /// Parse one strict release version: no prefix, suffix, sign or leading zero.
    ///
    /// # Errors
    /// Returns `pack-identity` for anything else.
    pub fn parse(value: &str) -> Result<Self> {
        let mut parts = value.split('.');
        let mut next = || parts.next().and_then(component);
        let (Some(major), Some(minor), Some(patch)) = (next(), next(), next()) else {
            return Err(identity("The version is not a plain release version."));
        };
        if parts.next().is_some() {
            return Err(identity("The version is not a plain release version."));
        }
        Ok(Self { major, minor, patch })
    }
}

fn component(text: &str) -> Option<u32> {
    let plain = !text.is_empty()
        && text.len() <= 9
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    if plain { text.parse().ok() } else { None }
}

impl fmt::Display for ReleaseVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The pack id and release version one folder or archive declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackIdentity {
    /// The `[project] name`, trimmed.
    pub name: String,
    /// The `[project] version`.
    pub version: ReleaseVersion,
}

impl PackIdentity {
    /// Parse the identity from `pyproject.toml` bytes.
    ///
    /// # Errors
    /// Returns `pack-identity` for oversized, unreadable or incomplete metadata.
    pub fn parse(pyproject: &[u8]) -> Result<Self> {
        if pyproject.len() > MAX_PYPROJECT_BYTES {
            return Err(identity("The pack metadata exceeds the size limit."));
        }
        let (name, version) = crate::installed_frontend::pack_project(pyproject)
            .map_err(|_| identity("The pack metadata is unreadable or incomplete."))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(identity("The pack metadata has no name."));
        }
        Ok(Self { name: name.to_owned(), version: ReleaseVersion::parse(&version)? })
    }

    /// Return whether this identity names the expected pack, compared as the format folds names.
    pub fn is_pack(&self, pack_id: &str) -> bool {
        super::entry::fold(&self.name) == super::entry::fold(pack_id)
    }
}

fn identity(detail: &'static str) -> PackInstallError {
    PackInstallError::format(ErrorCode::PackIdentity, detail)
}
