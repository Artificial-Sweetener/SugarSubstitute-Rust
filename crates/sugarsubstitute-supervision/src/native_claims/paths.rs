// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Installation and backend identities use shared retained path validation.
use super::*;
#[cfg(test)]
use crate::protected_storage::paths::{ancestor_paths, wide};
pub(crate) use crate::protected_storage::paths::{
    file_identity, final_path, open_retained, pin_ancestors, verify_pinned,
};
use sha2::{Digest, Sha256};
#[cfg(test)]
mod tests;
pub(crate) fn resolve(resolution: &mut Resolution, sid: &[u8]) -> Result<(), ClaimError> {
    let budget = files::Budget::Slot(resolution.slot.clone());
    let mut handles = lock(&resolution.handles);
    let root =
        open_retained(resolution.configuration.root(), true, false, budget.clone(), &mut handles)?;
    let root_path = final_path(&handles[root])?;
    let root_identity = file_identity(&handles[root])?;
    pin_ancestors(&root_path, &mut handles, files::Budget::Slot(resolution.slot.clone()))?;
    let interpreter =
        open_retained(resolution.configuration.interpreter(), false, false, budget, &mut handles)?;
    let interpreter_path = final_path(&handles[interpreter])?;
    let interpreter_identity = file_identity(&handles[interpreter])?;
    pin_ancestors(&interpreter_path, &mut handles, files::Budget::Slot(resolution.slot.clone()))?;
    verify_pinned(
        &root_path,
        root_identity,
        &mut handles,
        true,
        files::Budget::Slot(resolution.slot.clone()),
    )?;
    verify_pinned(
        &interpreter_path,
        interpreter_identity,
        &mut handles,
        false,
        files::Budget::Slot(resolution.slot.clone()),
    )?;
    let canonical = LaunchConfiguration::new(
        root_path,
        interpreter_path,
        resolution.configuration.arguments().to_vec(),
        resolution.configuration.port().get(),
    )
    .map_err(|e| ClaimError::new("canonical_configuration", e))?;
    resolution.digest = backend_hash(sid, &root_identity, &interpreter_identity, &canonical);
    drop(handles);
    resolution.configuration = canonical;
    resolution.account_sid = sid.to_vec();
    resolution.ready = true;
    Ok(())
}
fn backend_hash(
    sid: &[u8],
    root_identity: &[u8; 24],
    interpreter_identity: &[u8; 24],
    canonical: &LaunchConfiguration,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    for part in
        [b"SugarSubstitute.backend-claim.v1".as_slice(), sid, root_identity, interpreter_identity]
    {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.update((canonical.arguments().len() as u64).to_le_bytes());
    for argument in canonical.arguments() {
        hash.update((argument.len() as u64).to_le_bytes());
        hash.update(argument.as_bytes());
    }
    hash.update(canonical.port().get().to_le_bytes());
    hash.finalize().into()
}
