// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve maintenance configuration through protected accepted selection, independently of payload health.
use super::*;
use crate::setup_storage::{SelectionStore, protection};

/// Read selection without initializing it or mistaking damaged state for an earlier installation.
pub(super) fn generation(root: &Path, context: &JobContext) -> io::Result<Option<Generation>> {
    let Some(store) = SelectionStore::existing(root, context).map_err(io::Error::other)? else {
        return Ok(None);
    };
    let record = store
        .load(context)
        .map_err(io::Error::other)?
        .ok_or_else(|| invalid("Existing installation selection record is missing"))?;
    Ok(record.accepted().cloned())
}

/// Load only accepted saved settings; missing or damaged executables remain repairable.
/// Shell links are not configuration authority and are neither followed nor overwritten here.
pub(super) fn read(
    root: &Directory,
    marker: File,
    generation: &Generation,
    context: &JobContext,
) -> io::Result<ExistingInstallation> {
    let path = root.path.join("releases").join(generation.id.as_str());
    let directory = protection::with(context, |protection| {
        protection.directory(
            &path,
            crate::protected_storage::directories::Mode::Existing,
            context,
        )?;
        Directory::open(&path, false)
    })?;
    let mut existing =
        ExistingInstallation::from_directory(root, directory, marker, Some(generation))?;
    existing.accepted = Some(generation.clone());
    existing.summary.launch_entry_missing = match root.read("SugarSubstitute.lnk") {
        Ok(_) => false,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => true,
        Err(cause) => return Err(cause),
    };
    Ok(existing)
}
