// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish a stable root launcher independently of subsequent accepted application generations.
mod payload;
mod record;
use super::*;
use crate::setup_storage::{
    application::VerifiedApplication,
    check,
    protection::{Protection, Publication},
};
use sugarsubstitute_contracts::application_launch::LauncherContract;

pub(in crate::setup_storage) const EXECUTABLE: &str = "sugarsubstitute-launcher.exe";
const RECORD: &str = ".launcher-generation.json";

/// Called only under accepted application, setup and selection-writer custody.
/// Persist complete ownership before exposing an executable, and expose the shortcut last.
/// Retain the original launcher across application changes when its protocol still fits.
/// Returns false only for an older installation with neither launcher state nor payload.
pub(in crate::setup_storage) fn repair(
    root: &Directory,
    state: &Directory,
    application: &VerifiedApplication,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<bool> {
    check(context)?;
    let existing = record::read(state, protection)?;
    if existing.is_none() && !application.payload.compatibility.has_launcher() {
        return Ok(false);
    }
    validate_link(root, state, protection)?;
    let record = match existing {
        Some(record) => record,
        None => {
            match std::fs::symlink_metadata(root.path.join(EXECUTABLE)) {
                Ok(_) => {
                    return Err(invalid(
                        "The stable launcher path contains an unowned file; it has been preserved",
                    ));
                }
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause),
            }
            let record = record::Record::from_application(application, context)?;
            record::publish(state, &record, protection, context)?;
            record
        }
    };
    application.payload.compatibility.require_launcher(
        &LauncherContract::decode(record.contract.as_bytes()).map_err(io::Error::other)?,
    )?;
    let _executable = payload::ensure(root, &record, protection, context)?;
    let bytes = retained_link(root, state, protection, context)?;
    check(context)?;
    let replace = validate_link(root, state, protection)?;
    if replace && files::read_bounded(&mut root.read(NAME)?, 65536)? == bytes {
        return Ok(true);
    }
    let stage = staging::unique_path(root, ".launcher-link-stage")?;
    let mut output = protection.file(&stage, context)?;
    std::io::Write::write_all(&mut output, &bytes)?;
    output.sync_all()?;
    let identity = files::file_identity(&output)?;
    drop(output);
    check(context)?;
    protection.publish(
        &stage,
        &root.path.join(NAME),
        identity,
        Publication::File { replace },
        context,
    )?;
    Ok(true)
}

/// Require the currently installed launcher, independently of any launcher bundled in an offer.
pub(in crate::setup_storage) fn require_compatible(
    state: &Directory,
    compatibility: &crate::setup_storage::compatibility::Compatibility,
    protection: &Protection,
) -> io::Result<()> {
    if let Some(record) = record::read(state, protection)? {
        compatibility.require_launcher(
            &LauncherContract::decode(record.contract.as_bytes()).map_err(io::Error::other)?,
        )?;
    }
    Ok(())
}

/// Exact retained shell bytes distinguish our stable entry from user customizations.
fn retained_link(
    root: &Directory,
    state: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<Vec<u8>> {
    match state.read(NAME) {
        Ok(mut file) => {
            protection.verify_file(&file)?;
            if native::target(&state.path.join(NAME))? != root.path.join(EXECUTABLE) {
                return Err(invalid("Retained stable shortcut belongs to another target"));
            }
            files::read_bounded(&mut file, 65536)
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            let bytes = link_bytes(state, &root.path.join(EXECUTABLE))?;
            check(context)?;
            files::write_immutable(&state.path.join(NAME), &bytes, |path| {
                protection.file(path, context)
            })?;
            Ok(bytes)
        }
        Err(cause) => Err(cause),
    }
}

/// Validate before mutation and again before replacement; unknown links are never adopted.
fn validate_link(root: &Directory, state: &Directory, protection: &Protection) -> io::Result<bool> {
    let mut file = match root.read(NAME) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(cause) => return Err(cause),
    };
    if native::target(&root.path.join(NAME))? == root.path.join(EXECUTABLE) {
        let mut original = state.read(NAME)?;
        protection.verify_file(&original)?;
        if files::read_bounded(&mut file, 65536)? != files::read_bounded(&mut original, 65536)? {
            return Err(invalid(
                "The existing stable shortcut was modified; it has been preserved",
            ));
        }
    } else {
        selected(root)?.ok_or_else(|| invalid("Earlier shell entry disappeared"))?;
    }
    Ok(true)
}
