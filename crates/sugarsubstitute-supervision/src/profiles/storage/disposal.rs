// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Close only current acquisition outputs when no profile publication was issued.
use super::*;
#[cfg(test)]
mod tests;

/// Dispose a rejected acquisition through the same checked external close ports.
/// An issued, unknown or partially activated publication remains with release.
/// Predecessor bytes are neither read for release nor changed by this path.
pub(super) fn dispose_unpublished(
    state: &mut NativeState,
    context: &JobContext,
    close_file: impl FnOnce(&mut HeldFile<160>) -> Result<(), ClaimError>,
    close_root: impl FnOnce(&mut ProtectedStorageRoot) -> Result<(), ClaimError>,
) -> Result<bool, ClaimError> {
    if state.publication_attempted || state.record.is_some() || state.held {
        return Ok(false);
    }
    if state.release_published
        || state.environment.is_some()
        || state.browser.is_some()
        || state.prepared_folder.is_some()
        || state.migration_file.is_some()
        || state.desktop_custody.as_ref().is_some_and(|prepared| {
            prepared.prepared_nonce.is_some() || prepared.queued_prepare.is_some()
        })
        || state.file.as_ref().is_some_and(|file| {
            lock(&file.stage).is_some()
                || lock(&file.publication).as_ref().is_some_and(|binding| binding.targets_record())
        })
    {
        return Err(ClaimError::invalid("profile_unpublished_state_uncertain"));
    }
    check(context)?;
    if let Some(file) = state.file.as_mut() {
        close_file(file)?;
    }
    state.file = None;
    check(context)?;
    if let Some(root) = state.root.as_mut() {
        close_root(root)?;
    }
    state.root = None;
    state.held = false;
    state.retired = true;
    Ok(true)
}
