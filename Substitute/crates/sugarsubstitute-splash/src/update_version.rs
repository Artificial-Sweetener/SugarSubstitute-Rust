// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A short version label the splash can carry in its copyable appearance.

/// Longest version label the splash shows, in bytes.
pub const MAX_VERSION_BYTES: usize = 32;

/// The version an update is moving to, held inline so [`crate::Appearance`] stays `Copy`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpdateVersion {
    bytes: [u8; MAX_VERSION_BYTES],
    length: u8,
}

impl UpdateVersion {
    /// Hold `version`, or `None` when it is empty, longer than [`MAX_VERSION_BYTES`] or contains
    /// control characters, so a malformed label never reaches the screen.
    pub fn new(version: &str) -> Option<Self> {
        if version.is_empty()
            || version.len() > MAX_VERSION_BYTES
            || version.chars().any(char::is_control)
        {
            return None;
        }
        let mut bytes = [0; MAX_VERSION_BYTES];
        bytes[..version.len()].copy_from_slice(version.as_bytes());
        Some(Self { bytes, length: version.len() as u8 })
    }

    /// The version label.
    pub fn as_str(&self) -> &str {
        // The bytes were copied from a `str` and are cut only at its own length.
        std::str::from_utf8(&self.bytes[..usize::from(self.length)]).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_labels_round_trip_and_malformed_ones_are_refused() {
        assert_eq!(
            UpdateVersion::new("2.4.1").map(|v| v.as_str().to_owned()),
            Some("2.4.1".into())
        );
        assert_eq!(
            UpdateVersion::new("1.0.0-beta.12+build.7").map(|v| v.as_str().to_owned()),
            Some("1.0.0-beta.12+build.7".into())
        );
        assert!(UpdateVersion::new("").is_none());
        assert!(UpdateVersion::new(&"9".repeat(MAX_VERSION_BYTES + 1)).is_none());
        assert!(UpdateVersion::new("2.0\n").is_none());
        assert!(UpdateVersion::new(&"9".repeat(MAX_VERSION_BYTES)).is_some());
    }
}
