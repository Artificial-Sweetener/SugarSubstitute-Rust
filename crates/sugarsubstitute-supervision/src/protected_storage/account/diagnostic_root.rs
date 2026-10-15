// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin explicit diagnostic storage under the same actual primary-account and installation authority.
use super::*;
use crate::{coordinator_entry::DiagnosticStorageLocation, protected_storage::directories};
use sugarsubstitute_contracts::execution::JobContext;
/// Retain one actual private directory and its full native identity in original Entry custody.
pub(crate) struct PinnedDiagnosticDirectory {
    path: PathBuf,
    slot: usize,
    identity: [u8; 24],
}
impl ResolvedNamespace {
    /// Borrow the authoritative diagnostic root pin; account selection reuses its existing claims pin.
    pub(crate) fn diagnostic_directory(&self) -> (&Path, usize, [u8; 24]) {
        match &self.diagnostics {
            Some(root) => (&root.path, root.slot, root.identity),
            None => (&self.directory, self.directory_slot, self.directory_identity),
        }
    }
}
/// Resolve at most one explicit leaf after the actual installation/running-image association.
/// The parent must exist; unknown parents, foreign protection and partial outputs fail closed.
pub(crate) fn resolve(
    namespace: &mut ResolvedNamespace,
    location: &DiagnosticStorageLocation,
    handles: &mut Vec<Handle>,
    budget: Budget,
    context: &JobContext,
) -> Result<(), ClaimError> {
    let Some(requested) = location.explicit_path() else {
        return Ok(());
    };
    location.validate().map_err(|cause| ClaimError::new("diagnostic_root_input", cause))?;
    if namespace.diagnostics.is_some()
        || handles.len() >= handles.capacity()
        || handles.len() >= super::super::HANDLE_BOUND
    {
        return Err(ClaimError::invalid("diagnostic_root_capacity"));
    }
    let parent = requested.parent().ok_or_else(|| ClaimError::invalid("diagnostic_root_parent"))?;
    let leaf = requested.file_name().ok_or_else(|| ClaimError::invalid("diagnostic_root_leaf"))?;
    super::super::check(context)?;
    let required = paths::ancestor_paths(requested).map_err(ClaimError::invalid)?.count() + 4;
    if handles
        .len()
        .checked_add(required)
        .is_none_or(|count| count > super::super::HANDLE_BOUND || count > handles.capacity())
    {
        return Err(ClaimError::invalid("diagnostic_root_capacity"));
    }
    let mut phase = NativePhase::ordinary(&budget)?;
    paths::pin_ancestors(requested, handles, budget.clone(), &mut phase)?;
    let parent_slot =
        paths::open_retained(parent, true, true, budget.clone(), handles, &mut phase)?;
    let canonical = paths::final_path(
        &handles[parent_slot],
        &mut phase,
        super::super::native_name::NativeNameDestination::Ordinary,
    )?
    .into_ordinary()?;
    if local_drive(requested) != local_drive(&canonical) {
        return Err(ClaimError::invalid("diagnostic_root_volume"));
    }
    let identity = paths::file_identity(&handles[parent_slot], &mut phase)?;
    paths::verify_pinned(&canonical, identity, handles, true, budget.clone(), &mut phase)?;
    let path = canonical.join(leaf);
    super::super::check(context)?;
    let opened = directories::open_relative(
        handles,
        &namespace.account_sid,
        budget.clone(),
        directories::RelativeDirectory {
            path: &path,
            parent: parent_slot,
            mode: directories::Mode::OpenOrCreate,
            outputs: None,
        },
        context,
        &mut phase,
    )?;
    let path = paths::final_path(
        &handles[opened.index],
        &mut phase,
        super::super::native_name::NativeNameDestination::Ordinary,
    )?
    .into_ordinary()?;
    super::super::check(context)?;
    namespace.diagnostics =
        Some(PinnedDiagnosticDirectory { path, slot: opened.index, identity: opened.identity });
    Ok(())
}

fn local_drive(path: &Path) -> Option<u8> {
    use std::path::{Component, Prefix};
    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => Some(drive.to_ascii_uppercase()),
            _ => None,
        },
        _ => None,
    }
}
