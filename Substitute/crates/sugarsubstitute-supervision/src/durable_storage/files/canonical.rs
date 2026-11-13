// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve drive aliases while native ancestry pins prevent directory replacement.
use super::*;
use std::path::Prefix;

/// The local drive and the native object at every pinned component of one directory.
///
/// Two equal locations name the same directory reached from the same drive through the same
/// objects, whatever their spelling: letter case, 8.3 short-name aliases such as `RUNNER~1` and
/// the verbatim `\\?\` prefix all spell one location. A drive alias (`subst` or a redefined DOS
/// device) differs in its drive or its component objects. Reparse points never appear here,
/// because every pinned component was opened without following one.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Location {
    drive: u8,
    objects: Vec<[u8; 24]>,
}

impl Directory {
    /// Normalize a pinned local directory and verify the resolved spelling names the same object.
    pub(crate) fn resolved(self) -> io::Result<Self> {
        let canonical = std::fs::canonicalize(&self.path)?;
        let drive = local_drive(&canonical)
            .ok_or_else(|| invalid("Resolved directory must remain on a local drive"))?;
        let mut path = PathBuf::from(format!("{}:", char::from(drive)));
        for component in canonical.components().skip(1) {
            path.push(component);
        }
        let resolved = Self::open(&path, false)?;
        if resolved.identity()? != self.identity()? {
            return Err(invalid("Resolved directory identity changed"));
        }
        Ok(resolved)
    }

    /// Observe this directory's drive and the full native identity of every retained pin.
    /// The pins deny rename and deletion, so the observation stays true while `self` lives.
    /// # Errors
    /// Rejects a directory without a local drive and preserves native identity failures.
    pub(crate) fn location(&self) -> io::Result<Location> {
        let drive = local_drive(&self.path)
            .ok_or_else(|| invalid("Directory must be on a local drive"))?
            .to_ascii_uppercase();
        let objects = self._pins.iter().map(native_identity).collect::<io::Result<_>>()?;
        Ok(Location { drive, objects })
    }
}

/// Read the drive letter of a DOS or verbatim drive path; other prefixes have none.
fn local_drive(path: &Path) -> Option<u8> {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => Some(drive),
            _ => None,
        },
        _ => None,
    }
}
