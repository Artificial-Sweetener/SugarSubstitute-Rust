// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish a stable root launcher independently of subsequent accepted application generations.
mod payload;
mod record;
mod transition;
use super::*;
use crate::{
    durable_storage::protection::Protection,
    setup_storage::{application::VerifiedApplication, check, publication},
};
use sugarsubstitute_contracts::application_launch::LauncherContract;

pub(in crate::setup_storage) const EXECUTABLE: &str = "sugarsubstitute-launcher.exe";
const RECORD: &str = ".launcher-generation.json";

/// Recognize protected stable entry ownership for settings recovery independently of payload health.
/// Missing owned links are repairable. Unknown formats and edited links remain errors.
/// The launcher source is not configuration selection or application launch authority.
pub(in crate::setup_storage) fn owned_entry(
    root: &Directory,
    state: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<Option<bool>> {
    if record::read(state, protection, context)?.is_none() {
        return Ok(None);
    }
    validate_link(root, state, protection, context).map(Some)
}

/// Restore the independently selected launcher under accepted application, setup and writer custody.
/// Application acceptance never rolls back a launcher selected by a newer authenticated update.
/// Returns false for a historical installation with neither launcher ownership nor payload.
pub(in crate::setup_storage) fn repair(
    root: &Directory,
    state: &Directory,
    application: &VerifiedApplication,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<bool> {
    recover(root, state, protection, context)?;
    let existing = record::read(state, protection, context)?;
    if existing.is_none() && !application.payload.compatibility.has_launcher() {
        return Ok(false);
    }
    validate_link(root, state, protection, context)?;
    let record = match existing {
        Some(record) => record,
        None => initialize(
            root,
            state,
            record::Record::from_source(&application.generation, &application.directory, context)?,
            protection,
            context,
        )?,
    };
    application.payload.compatibility.require_launcher(
        &LauncherContract::decode(record.contract.as_bytes()).map_err(io::Error::other)?,
    )?;
    publish_owned(root, state, &record, protection, context)?;
    Ok(true)
}

/// Publish first-launch entries only for an explicitly authorized prepared installation.
/// Established ownership uses the same recoverable replacement transaction as signed updates.
pub(in crate::setup_storage) fn install(
    root: &Directory,
    state: &Directory,
    candidate: &crate::setup_storage::PreparedCandidate,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    recover_publications(root, state, protection, context)?;
    if record::read(state, protection, context)?.is_some() {
        return prepare(root, state, candidate, protection, context);
    }
    transition::recover(root, state, protection, context)?;
    validate_link(root, state, protection, context)?;
    let directory = Directory::open(candidate.directory(), false)?;
    let record = record::Record::from_source(candidate.generation(), &directory, context)?;
    candidate.compatibility().require_launcher(
        &LauncherContract::decode(record.contract.as_bytes()).map_err(io::Error::other)?,
    )?;
    let record = initialize(root, state, record, protection, context)?;
    publish_owned(root, state, &record, protection, context)
}

/// Establish create-only ownership before copying any executable; preserve unknown root files.
fn initialize(
    root: &Directory,
    state: &Directory,
    record: record::Record,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<record::Record> {
    match files::inspect(root.path().join(EXECUTABLE), context) {
        Ok(_) => {
            return Err(invalid(
                "The stable launcher path contains an unowned file; it has been preserved",
            ));
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    record::publish(state, &record, protection, context)?;
    Ok(record)
}

/// Advance established launcher ownership using a completed candidate under current signed authority.
/// Retain the previous executable until the replacement intent is durable. Missing original source
/// files do not prevent rescue from the authenticated candidate; unknown root files remain intact.
pub(in crate::setup_storage) fn prepare(
    root: &Directory,
    state: &Directory,
    candidate: &crate::setup_storage::PreparedCandidate,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    recover_publications(root, state, protection, context)?;
    if !candidate.compatibility().has_launcher()
        || record::read(state, protection, context)?.is_none()
    {
        transition::recover(root, state, protection, context)?;
        let Some(record) = record::read(state, protection, context)? else { return Ok(()) };
        validate_link(root, state, protection, context)?;
        return publish_owned(root, state, &record, protection, context);
    }
    validate_link(root, state, protection, context)?;
    let directory = Directory::open(candidate.directory(), false)?;
    let next = record::Record::from_source(candidate.generation(), &directory, context)?;
    candidate.compatibility().require_launcher(
        &LauncherContract::decode(next.contract.as_bytes()).map_err(io::Error::other)?,
    )?;
    let record = transition::supersede(root, state, next, protection, context)?;
    publish_owned(root, state, &record, protection, context)
}

/// Recover publication temporaries before resuming a durable launcher transition.
fn recover(
    root: &Directory,
    state: &Directory,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    recover_publications(root, state, protection, context)?;
    transition::recover(root, state, protection, context)?;
    Ok(())
}

/// Settle atomic-file publication before inspecting either ordinary or superseding intent.
fn recover_publications(
    root: &Directory,
    state: &Directory,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    check(context)?;
    publication::recover(root, protection, context)?;
    publication::recover(state, protection, context)?;
    Ok(())
}

/// Publish the exact recorded executable and retained shortcut without changing ownership.
fn publish_owned(
    root: &Directory,
    state: &Directory,
    record: &record::Record,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    let _executable = payload::ensure(root, record, protection, context)?;
    let bytes = retained_link(root, state, protection, context)?;
    check(context)?;
    let replace = validate_link(root, state, protection, context)?;
    if replace && files::read_bounded(&mut root.read(NAME)?, 65536)? == bytes {
        return Ok(());
    }
    let mut stage = publication::Stage::create(root, protection, context)?;
    std::io::Write::write_all(stage.file(), &bytes)?;
    check(context)?;
    stage.publish(&root.path().join(NAME), replace, protection, context)?;
    Ok(())
}

/// Require the currently installed launcher, independently of any launcher bundled in an offer.
pub(in crate::setup_storage) fn require_compatible(
    state: &Directory,
    compatibility: &crate::setup_storage::compatibility::Compatibility,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<()> {
    if let Some(record) = record::read(state, protection, context)? {
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
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<Vec<u8>> {
    match state.read(NAME) {
        Ok(mut file) => {
            protection.verify_file(&file)?;
            if native::target(&state.path().join(NAME))? != root.path().join(EXECUTABLE) {
                return Err(invalid("Retained stable shortcut belongs to another target"));
            }
            files::read_bounded(&mut file, 65536)
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            let bytes = native::encode(&root.path().join(EXECUTABLE))?;
            check(context)?;
            publication::write_immutable(state, NAME, &bytes, protection, context)?;
            Ok(bytes)
        }
        Err(cause) => Err(cause),
    }
}

/// Validate before mutation and again before replacement; unknown links are never adopted.
pub(in crate::setup_storage) fn validate_link(
    root: &Directory,
    state: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<bool> {
    check(context)?;
    let mut file = match root.read(NAME) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(cause) => return Err(cause),
    };
    if native::target(&root.path().join(NAME))? == root.path().join(EXECUTABLE) {
        let mut original = state.read(NAME)?;
        protection.verify_file(&original)?;
        if files::read_bounded(&mut file, 65536)? != files::read_bounded(&mut original, 65536)? {
            return Err(invalid(
                "The existing stable shortcut was modified; it has been preserved",
            ));
        }
    } else {
        selected(root, context)?.ok_or_else(|| invalid("Earlier shell entry disappeared"))?;
    }
    Ok(true)
}
