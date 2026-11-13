// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish a stable root launcher independently of subsequent accepted application generations.
//!
//! The root's only file is `SugarSubstitute.exe`, a protected copy of an accepted generation's
//! launcher. Users start it directly; no shell shortcut is published. Earlier installations
//! kept a `sugarsubstitute-launcher.exe` copy and a `SugarSubstitute.lnk` shortcut in the
//! root; publication retires both once each is verified to be exactly ours.
mod payload;
mod record;
mod transition;
use super::*;
use crate::{
    durable_storage::protection::Protection,
    setup_storage::{application::VerifiedApplication, check, launch_pointer, publication},
};
use sugarsubstitute_contracts::application_launch::LauncherContract;

/// Launcher image name inside each application generation.
pub(in crate::setup_storage) const SOURCE: &str = "sugarsubstitute-launcher.exe";
/// Stable entry published at the installation root.
pub(in crate::setup_storage) const ENTRY: &str = sugarsubstitute_configuration::layout::ENTRY;
/// Root launcher copy of the earlier layout, retired after the entry is published.
const EARLIER_ENTRY: &str = SOURCE;
const RECORD: &str = ".launcher-generation.json";

/// Recognize protected stable entry ownership for settings recovery independently of payload health.
/// Returns whether the owned entry is present; a missing entry is repairable and unknown files
/// at the entry path remain errors. The launcher source is not configuration selection or
/// application launch authority.
pub(in crate::setup_storage) fn owned_entry(
    root: &Directory,
    state: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<Option<bool>> {
    if record::read(state, protection, context)?.is_none() {
        return Ok(None);
    }
    validate_entry(root, state, protection, context).map(Some)
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
    ensure_pointer(root, &application.generation.id, &application.directory, protection, context)?;
    Ok(true)
}

/// Publish the first-launch entry only for an explicitly authorized prepared installation.
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
    let directory = Directory::open(candidate.directory(), false)?;
    let record = record::Record::from_source(candidate.generation(), &directory, context)?;
    candidate.compatibility().require_launcher(
        &LauncherContract::decode(record.contract.as_bytes()).map_err(io::Error::other)?,
    )?;
    let record = initialize(root, state, record, protection, context)?;
    publish_owned(root, state, &record, protection, context)?;
    point(root, &candidate.generation().id, &directory, protection, context)
}

/// Point the stable entry at this generation's launch engine, replacing any earlier pointer.
fn point(
    root: &Directory,
    generation: &sugarsubstitute_contracts::installation::GenerationId,
    directory: &Directory,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    let state = Directory::open(&crate::setup_storage::ownership::state_path(root), false)?;
    let primary = launch_pointer::EngineTarget::of(generation, directory, context)?;
    launch_pointer::publish(
        &state,
        &launch_pointer::LaunchPointer::new(primary, None),
        protection,
        context,
    )
}

/// Repair a missing or damaged pointer from the accepted, running generation. A readable pointer,
/// or one a newer release wrote in a format this release cannot read, is left alone, because only
/// the installer and redirecting engines choose what the stable entry starts.
fn ensure_pointer(
    root: &Directory,
    generation: &sugarsubstitute_contracts::installation::GenerationId,
    directory: &Directory,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    let state = Directory::open(&crate::setup_storage::ownership::state_path(root), false)?;
    match launch_pointer::read(&state, protection) {
        Err(launch_pointer::PointerFailure::Missing | launch_pointer::PointerFailure::Damaged) => {
            point(root, generation, directory, protection, context)
        }
        Ok(_)
        | Err(
            launch_pointer::PointerFailure::Incompatible
            | launch_pointer::PointerFailure::EngineMismatch,
        ) => Ok(()),
        Err(launch_pointer::PointerFailure::Storage(cause)) => Err(cause),
    }
}

/// Establish create-only ownership before copying any executable; preserve unknown root files.
fn initialize(
    root: &Directory,
    state: &Directory,
    record: record::Record,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<record::Record> {
    match files::inspect(root.path().join(ENTRY), context) {
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
        return publish_owned(root, state, &record, protection, context);
    }
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

/// Publish the exact recorded executable as the root entry without changing ownership, then
/// retire the earlier layout's root launcher copy and shortcut when they are verifiably ours.
fn publish_owned(
    root: &Directory,
    state: &Directory,
    record: &record::Record,
    protection: &mut Protection,
    context: &JobContext,
) -> io::Result<()> {
    let _executable = payload::ensure(root, state, record, protection, context)?;
    check(context)?;
    payload::retire_earlier(root, EARLIER_ENTRY, record, protection, context)?;
    retire_earlier_shortcut(root, state, protection, context)
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

/// Report whether the owned root entry is present. A file at the entry path without this
/// installation's ownership or protection is never adopted and remains an error.
pub(in crate::setup_storage) fn validate_entry(
    root: &Directory,
    state: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<bool> {
    check(context)?;
    let file = match root.read(ENTRY) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(cause) => return Err(cause),
    };
    if record::read(state, protection, context)?.is_none() {
        return Err(invalid(
            "The stable launcher path contains an unowned file; it has been preserved",
        ));
    }
    protection.verify_file(&file)?;
    Ok(true)
}

/// Retire the earlier root shortcut only while it still has the exact bytes retained when it was
/// published: beside the stable launcher record for a launcher shortcut, or in its generation for
/// a historical shortcut that opens the application directly. Edited and foreign files remain.
fn retire_earlier_shortcut(
    root: &Directory,
    state: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<()> {
    check(context)?;
    let mut file = match root.read(NAME) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(cause),
    };
    let owned = match native::target(&root.path().join(NAME)) {
        Ok(target) if target == root.path().join(EARLIER_ENTRY) => {
            let mut retained = match state.read(NAME) {
                Ok(file) => file,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(cause) => return Err(cause),
            };
            protection.verify_file(&retained).is_ok()
                && files::read_bounded(&mut file, 65536)?
                    == files::read_bounded(&mut retained, 65536)?
        }
        // A historical installation's shortcut opens a generation directly; its generation
        // retains the exact bytes that prove it is ours.
        Ok(_) => selected(root, context).is_ok_and(|directory| directory.is_some()),
        Err(_) => false,
    };
    if !owned {
        tracing::warn!(
            operation = "stable_shortcut_retirement",
            "The earlier root shortcut is not exactly ours; it has been preserved"
        );
        return Ok(());
    }
    files::single_link(&file)?;
    let identity = files::file_identity(&file)?;
    drop(file);
    tracing::info!(operation = "stable_shortcut_retirement", "Retiring the earlier root shortcut");
    crate::durable_storage::retirement::file(root, NAME, identity)
}
