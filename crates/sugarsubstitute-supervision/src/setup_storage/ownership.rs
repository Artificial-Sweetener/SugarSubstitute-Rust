// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read and pin the native installation ownership marker through maintenance operations.
use super::{
    files::{self, Directory},
    invalid,
};
use std::{fs::File, io};
pub(super) const MARKER: &str = ".substitute-installation.json";
pub(super) const CONTENT: &[u8] = br#"{"schema_version":1,"product":"SugarSubstitute"}"#;

/// Validate the complete marker and retain its handle against replacement or modification.
pub(super) fn read(root: &Directory) -> io::Result<File> {
    let mut file = root.read(MARKER)?;
    let value: serde_json::Value =
        serde_json::from_slice(&files::read_bounded(&mut file, 4096)?).map_err(io::Error::other)?;
    if value != serde_json::json!({"schema_version":1,"product":"SugarSubstitute"}) {
        return Err(invalid("Installation ownership could not be verified"));
    }
    Ok(file)
}
