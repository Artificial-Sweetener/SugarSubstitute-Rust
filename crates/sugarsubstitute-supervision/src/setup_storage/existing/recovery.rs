// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover unambiguous saved configuration without selecting or executing an old payload.
use super::*;

const MAX_ENTRIES: usize = 1024;

/// Saved-data recovery needs a decision that automatic inspection cannot safely make.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryFailure {
    /// Completed generations disagree about user configuration.
    ConflictingSettings,
    /// Existing files contain no completed configuration to preserve.
    IncompleteSettings,
    /// The bounded discovery operation cannot inspect the whole inventory.
    InventoryBound,
}
impl std::fmt::Display for RecoveryFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Native installation recovery rejected: {self:?}")
    }
}
impl std::error::Error for RecoveryFailure {}

/// A missing entry permits bounded data recovery, never a guessed "newest" executable.
/// Incomplete generations and unrelated files remain untouched. Every completed record must
/// agree on saved configuration before the installer can offer automatic repair.
pub(super) fn discover(
    root: &Directory,
    marker: File,
    context: &JobContext,
) -> io::Result<Option<ExistingInstallation>> {
    let releases = root.path().join("releases");
    let directory = match Directory::open(&releases, false) {
        Ok(directory) => directory,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => return Err(cause),
    };
    let mut recovered: Option<ExistingInstallation> = None;
    let mut occupied = false;
    for (index, entry) in std::fs::read_dir(directory.path())?.enumerate() {
        check(context)?;
        if index >= MAX_ENTRIES {
            return Err(io::Error::other(RecoveryFailure::InventoryBound));
        }
        let entry = entry?;
        occupied = true;
        let kind = entry.file_type()?;
        if kind.is_file() {
            continue;
        }
        let generation = Directory::open(&entry.path(), false)?;
        match generation.read("installation-ready.json") {
            Ok(_) => {}
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => continue,
            Err(cause) => return Err(cause),
        }
        let mut candidate =
            ExistingInstallation::from_directory(root, generation, marker.try_clone()?, None)?;
        candidate.summary.launch_entry_missing = true;
        if let Some(previous) = &recovered {
            if !equivalent(previous, &candidate) {
                return Err(io::Error::other(RecoveryFailure::ConflictingSettings));
            }
            if previous._directory.path() <= candidate._directory.path() {
                continue;
            }
        }
        recovered = Some(candidate);
    }
    check(context)?;
    if occupied && recovered.is_none() {
        return Err(io::Error::other(RecoveryFailure::IncompleteSettings));
    }
    Ok(recovered)
}

/// Ignore only the checked generation-local configuration path and argument ordering.
/// User-authored startup bytes and every other launch/Comfy selection remain exact.
fn equivalent(left: &ExistingInstallation, right: &ExistingInstallation) -> bool {
    left.startup == right.startup
        && left.runtime == right.runtime
        && left.summary.selection == right.summary.selection
        && left.descriptor.same_external_inputs(&right.descriptor)
}

#[cfg(test)]
mod tests;
