// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate inert selected paths before any receiving-file effect.
use super::{ReportFileCode as C, ReportFileError as E};
/// Fixed immutable selected path and overwrite decision; grants no native authority.
pub struct ReportFileSelection {
    pub(super) overwrite: bool,
    pub(super) path: [u16; 1025],
    pub(super) length: usize,
    pub(super) ends: [usize; 64],
    pub(super) directories: usize,
    pub(super) identity: [u64; 2],
}
impl ReportFileSelection {
    /// Decode exact little-endian UTF16 without changing user-selected spelling.
    /// `identity` supplies the authenticated session/request pair used to isolate
    /// stage and predecessor sibling names. `overwrite` is the feature's explicit
    /// consent decision; neither value authorizes an OS effect without real admission.
    /// Local drive paths allow at most 64 parents and 1024 UTF16 units.
    /// # Errors
    /// Returns Destination for empty/odd/oversized bytes, malformed Unicode, device or
    /// UNC paths, ADS, reserved names, ambiguous components or excessive ancestry.
    pub fn new(bytes: &[u8], overwrite: bool, identity: [u64; 2]) -> Result<Self, E> {
        if bytes.is_empty() || !bytes.len().is_multiple_of(2) || bytes.len() > 2048 {
            return Err(E::Rejected(C::Destination));
        }
        let mut selection = Self {
            overwrite,
            identity,
            path: [0; 1025],
            length: bytes.len() / 2,
            ends: [0; 64],
            directories: 0,
        };
        for (unit, pair) in selection.path.iter_mut().zip(bytes.chunks_exact(2)) {
            *unit = u16::from_le_bytes([pair[0], pair[1]]);
        }
        selection.validate()?;
        Ok(selection)
    }
    /// Retain only unambiguous local paths and their fixed ancestor end offsets.
    fn validate(&mut self) -> Result<(), E> {
        let path = &self.path[..self.length];
        if path.len() < 4
            || !matches!(path[0], 65..=90 | 97..=122)
            || path[1] != 58
            || path[2] != 92
            || char::decode_utf16(path.iter().copied()).any(|unit| unit.is_err())
        {
            return Err(E::Rejected(C::Destination));
        }
        self.ends[0] = 3;
        self.directories = 1;
        let mut start = 3;
        for end in 3..=path.len() {
            if end != path.len() && path[end] != 92 {
                continue;
            }
            if !component(&path[start..end]) {
                return Err(E::Rejected(C::Destination));
            }
            if end != path.len() {
                if self.directories == self.ends.len() {
                    return Err(E::Rejected(C::Destination));
                }
                self.ends[self.directories] = end;
                self.directories += 1;
            }
            start = end + 1;
        }
        Ok(())
    }
}
/// Reject Win32 aliases, ADS, wildcard syntax and trailing-dot/space ambiguity.
fn component(units: &[u16]) -> bool {
    if units.is_empty()
        || units.len() > 255
        || matches!(units.last(), Some(32 | 46))
        || units
            .iter()
            .any(|unit| *unit < 32 || matches!(unit, 34 | 42 | 47 | 58 | 60 | 62 | 63 | 124))
    {
        return false;
    }
    let stem = &units[..units.iter().position(|unit| *unit == 46).unwrap_or(units.len())];
    let mut ascii = [0; 7];
    if stem.len() > ascii.len() {
        return true;
    }
    for (destination, unit) in ascii.iter_mut().zip(stem) {
        *destination = match unit {
            97..=122 => *unit - 32,
            _ => *unit,
        };
    }
    !matches!(
        &ascii[..stem.len()],
        [67, 79, 78]
            | [80, 82, 78]
            | [65, 85, 88]
            | [78, 85, 76]
            | [67, 79, 78, 73, 78, 36]
            | [67, 79, 78, 79, 85, 84, 36]
            | [67, 79, 77, 49..=57 | 178 | 179 | 185]
            | [76, 80, 84, 49..=57 | 178 | 179 | 185]
    )
}
#[cfg(test)]
mod tests;
