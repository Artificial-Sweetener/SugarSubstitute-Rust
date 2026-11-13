// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project only the native-selected child profile through the authoritative Launch grammar.
use super::*;
use std::{ops::Range, path::Path};
use sugarsubstitute_contracts::{
    configuration::LaunchConfiguration,
    coordinator_entry::{EntryShapeError, MAX_ENTRY_PATH_UNITS},
};
fn projection(body: &[u8], profile: &Path) -> Result<(Range<usize>, usize), EntryTransportError> {
    let (_, original) = decode::with_profile(body)?;
    LaunchConfiguration::validate_path(profile)
        .map_err(|cause| EntryTransportError::Shape(EntryShapeError::Configuration(cause)))?;
    let path_bytes = raw_windows::length(profile)?;
    if path_bytes > MAX_ENTRY_PATH_UNITS * 2 {
        return Err(EntryTransportError::Capacity);
    }
    let length = body
        .len()
        .checked_sub(original.len())
        .and_then(|length| length.checked_add(4))
        .and_then(|length| length.checked_add(path_bytes))
        .filter(|length| *length <= MAX_BODY_BYTES)
        .ok_or(EntryTransportError::Capacity)?;
    Ok((original, length))
}
/// Measure a selected profile substitution within the unchanged whole-body limit.
/// The original complete grammar and the selected absolute path are validated first.
pub(crate) fn profile_length(body: &[u8], profile: &Path) -> Result<usize, EntryTransportError> {
    projection(body, profile).map(|(_, length)| length)
}
/// Encode only the selected profile; preserve every other original byte exactly.
/// Rejects shape, growth and capacity before touching caller-owned output.
/// This inert projection grants no profile, filesystem or process authority.
pub(crate) fn project_profile(
    output: &mut [u8],
    body: &[u8],
    profile: &Path,
) -> Result<usize, EntryTransportError> {
    let (original, length) = projection(body, profile)?;
    let output = output.get_mut(..length).ok_or(EntryTransportError::Capacity)?;
    let mut writer = Writer::new(output);
    writer.bytes(&body[..original.start])?;
    encode::path(&mut writer, profile)?;
    writer.bytes(&body[original.end..])?;
    Ok(length)
}
