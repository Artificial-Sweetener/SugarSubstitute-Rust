// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate bounded native-selected path data before any receiving-file effect.
use super::{ReportExportCause as E, ReportExportCode as C};
use crate::diagnostic_client::{DiagnosticRequestId, DiagnosticSelection};
use sugarsubstitute_contracts::incident::{ExportSelection, ReportBinding};
/// Owned immutable bounded selection; construction authenticates no sender.
pub struct ReportExportIntent {
    pub(super) binding: ReportBinding,
    pub(super) request: DiagnosticRequestId,
    pub(super) selection: ExportSelection,
    pub(crate) overwrite: bool,
    pub(crate) path: [u16; 1025],
    pub(crate) length: usize,
    pub(crate) ends: [usize; 64],
    pub(crate) directories: usize,
}
impl ReportExportIntent {
    /// Decode exactly the authenticated selection's little-endian UTF16 data.
    /// The adapter must match the selected request against its current session ledger.
    /// Local absolute drive paths contain at most64 parent components and1024 units;
    /// device/UNC paths, ADS, dot components, reserved names and ambiguous suffixes reject.
    /// Unicode scalar pairs are validated without changing user-selected spelling.
    /// # Errors
    /// Returns Destination before any allocation/native effect for malformed paths.
    pub fn new(binding: ReportBinding, selection: DiagnosticSelection<'_>) -> Result<Self, E> {
        let bytes = selection.destination;
        if bytes.is_empty() || !bytes.len().is_multiple_of(2) || bytes.len() > 2048 {
            return Err(E::Rejected(C::Destination));
        }
        let mut intent = Self {
            binding,
            request: selection.id,
            selection: selection.selection,
            overwrite: selection.overwrite,
            path: [0; 1025],
            length: bytes.len() / 2,
            ends: [0; 64],
            directories: 0,
        };
        for (unit, pair) in intent.path.iter_mut().zip(bytes.chunks_exact(2)) {
            *unit = u16::from_le_bytes([pair[0], pair[1]]);
        }
        intent.validate()?;
        Ok(intent)
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
    /// Return full incident/revision/session matching facts.
    pub const fn binding(&self) -> ReportBinding {
        self.binding
    }
    /// Return exact original selected request number, never filesystem authority.
    pub const fn request(&self) -> DiagnosticRequestId {
        self.request
    }
    /// Return actual sensitive dump consent selection.
    pub const fn selection(&self) -> ExportSelection {
        self.selection
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
