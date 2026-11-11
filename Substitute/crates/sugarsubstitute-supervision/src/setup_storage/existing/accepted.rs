// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve maintenance configuration through protected accepted selection, independently of payload health.
use super::*;
use crate::{durable_storage::protection, setup_storage::SelectionStore};

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
    let path = root.path().join("releases").join(generation.id.as_str());
    let directory = protection::with(context, |protection| {
        protection.directory(
            &path,
            crate::durable_storage::protected::directories::Mode::Existing,
            context,
        )?;
        Directory::open(&path, false)
    })?;
    let mut existing =
        ExistingInstallation::from_directory(root, directory, marker, Some(generation), context)?;
    existing.accepted = Some(generation.clone());
    // The stable root entry, or a historical installation's generation shortcut, opens the app.
    existing.summary.launch_entry_missing = true;
    for leaf in [crate::setup_storage::entrypoint::stable::ENTRY, "SugarSubstitute.lnk"] {
        match root.read(leaf) {
            Ok(_) => existing.summary.launch_entry_missing = false,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
            Err(cause) => return Err(cause),
        }
    }
    Ok(existing)
}
