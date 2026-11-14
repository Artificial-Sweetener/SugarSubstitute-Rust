// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Set update storage another program made untrustworthy aside and start trust again.
//!
//! Protected update trust is only as good as its security. When another account can change it,
//! its DACL inherits again, its owner changed or its label was lowered, nothing in it can be
//! relied on. The storage is moved, never deleted, into a set-aside folder under `user\` for
//! diagnosis, and fresh storage this account owns is created from the trusted root this build
//! ships. The fresh storage is staged complete before anything moves, so a reset interrupted at
//! any point is finished by the next one. Only the newest [`KEPT_SET_ASIDE`] set-aside folders are kept. Rollback protection
//! holds because the fresh release history starts each application route at the highest release
//! the protected selection knows and each updater route at this build's own engine release:
//! nothing older is accepted again. The next check downloads as usual.
use super::*;
use crate::setup_storage::check;
use std::path::PathBuf;
use sugarsubstitute_contracts::{
    installation::{Candidate, Generation, InstallationRecord},
    update::{Channel, Component},
};
use sugarsubstitute_update::{ENGINE_RELEASE, ReleaseHistory, RouteStart};

/// Folder under the installation's `user` folder that keeps set-aside update storage.
pub const SET_ASIDE_FOLDER: &str = "update-storage-set-aside";
/// Set-aside folders kept for diagnosis; older ones are deleted after each reset.
const KEPT_SET_ASIDE: usize = 2;
#[cfg(test)]
thread_local! {
    /// Stops a reset on this thread right after it set the old storage aside.
    pub(crate) static INTERRUPT_AFTER_SET_ASIDE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}
/// Name prefix of each numbered set-aside folder.
const SET_ASIDE_PREFIX: &str = "set-aside";
/// Fresh trust staged complete in the state folder, waiting to replace the set-aside storage.
const READY: &str = ".update-trust-reset";
/// Prefix of the folders fresh trust is staged in before it is ready.
const RESET_STAGE: &str = ".update-trust-reset-stage";
/// The installation's update workspace, where downloads and extraction are staged.
const WORKSPACE: &str = ".update-workspace";

impl TrustStore {
    /// Whether `failure` means the protected update storage has security that no longer keeps
    /// other accounts out, so it must be set aside rather than retried.
    pub fn storage_untrusted(failure: &TrustFailure) -> bool {
        crate::durable_storage::protected::security::untrusted(failure)
    }

    /// Whether a reset stopped after it set the old storage aside and before its fresh trust
    /// replaced it; the next [`Self::reset_untrusted`] finishes it.
    /// # Errors
    /// The state folder cannot be inspected.
    pub fn reset_interrupted(installation_root: &Path, context: &JobContext) -> io::Result<bool> {
        let state =
            sugarsubstitute_configuration::layout::InstallationLayout::new(installation_root)
                .state();
        Ok(!present(&state.join(DIRECTORY), context)? && present(&state.join(READY), context)?)
    }

    /// Set untrusted update storage aside and create fresh trust from `trusted_root`, the root
    /// metadata this build ships. Returns the store and whether its storage was set up again. Storage whose DACL only inherits again while still letting in no one but
    /// this account and SYSTEM is protected again in place and kept instead. Call only after
    /// [`Self::storage_untrusted`] judged an open of this installation's trust; storage that opens
    /// once this reset holds the selection writer is kept as it is. The accepted
    /// release keeps starting throughout: selection is read, never changed. Fresh trust is
    /// staged complete before anything is moved; a reset that [`Self::reset_interrupted`]
    /// publishes the trust it staged. Run on a storage worker reserving
    /// `TRUST_RESERVATION_BYTES`.
    /// # Errors
    /// An invalid root, storage still in use by another process (retried by the next check),
    /// and storage failures. Nothing is deleted on any path.
    pub fn reset_untrusted(
        installation_root: &Path,
        repository: [u8; 32],
        trusted_root: &[u8],
        context: &JobContext,
    ) -> Result<(Self, bool), TrustFailure> {
        check(context)?;
        let store = SelectionStore::open(installation_root, context).map_err(io::Error::other)?;
        let (selection, _writer) = store.hold_writer(context).map_err(io::Error::other)?;
        let interrupted = Self::reset_interrupted(installation_root, context)?;
        if !interrupted {
            // Another check may have set the storage up again while this one waited.
            if let Some(trust) = Self::reopen(installation_root, repository, &selection, context)? {
                return Ok((trust, false));
            }
            if reprotect_harmless(installation_root, context)?
                && let Some(trust) =
                    Self::reopen(installation_root, repository, &selection, context)?
            {
                tracing::warn!(
                    operation = "update_storage_reprotected",
                    "Update storage that inherited permissions again was protected again"
                );
                return Ok((trust, false));
            }
            stage_fresh(installation_root, repository, trusted_root, &selection, context)?;
        }
        let set_aside = set_aside(installation_root, context)?;
        #[cfg(test)]
        if INTERRUPT_AFTER_SET_ASIDE.get() {
            return Err(invalid("The reset was interrupted").into());
        }
        publish_fresh(installation_root, context)?;
        let trust = Self::reopen(installation_root, repository, &selection, context)?
            .ok_or_else(|| invalid("Fresh update trust is not protected"))?;
        match &set_aside {
            Some(folder) => tracing::warn!(operation = "update_storage_set_aside",
                folder = %folder.display(),
                "Update storage another program changed was set aside; updates were set up again"),
            None => tracing::warn!(
                operation = "update_storage_reset_finished",
                "An interrupted reset of update storage was finished; updates were set up again"
            ),
        }
        Ok((trust, true))
    }

    /// Open this installation's trust for `selection`, whose writer the caller holds, or `None`
    /// while its storage is still untrusted.
    fn reopen(
        installation_root: &Path,
        repository: [u8; 32],
        selection: &InstallationRecord,
        context: &JobContext,
    ) -> Result<Option<Self>, TrustFailure> {
        match Self::open_selected(installation_root, Some(repository), selection, context) {
            Ok(trust) => Ok(Some(trust)),
            Err(cause) if Self::storage_untrusted(&cause) => Ok(None),
            Err(cause) => Err(cause),
        }
    }
}

/// Release history for fresh trust: application routes start at the highest release `selection`
/// knows (running, retained or prepared), so a superseded offer below an already authorised
/// release stays refused. Updater routes start at this build's own engine release; the installed
/// engine's records were set aside with the storage, so its package is not known.
fn starting_history(selection: &InstallationRecord) -> io::Result<ReleaseHistory> {
    let platform = UpdateAuthorization::local_request(Channel::Release)?.platform;
    let highest = [
        selection.accepted(),
        selection.retained(),
        selection.candidate().map(Candidate::generation),
    ]
    .into_iter()
    .flatten()
    .max_by_key(|generation| generation.release.sequence());
    let (sequence, digest) = match highest {
        Some(generation) => (generation.release.sequence(), Some(package_digest(generation)?)),
        None => (0, None),
    };
    let starts: Vec<_> = [Channel::Release, Channel::Canary]
        .into_iter()
        .flat_map(|channel| {
            [
                RouteStart {
                    channel,
                    component: Component::Application,
                    platform,
                    sequence,
                    digest,
                },
                RouteStart {
                    channel,
                    component: Component::Updater,
                    platform,
                    sequence: ENGINE_RELEASE,
                    digest: None,
                },
            ]
        })
        .collect();
    ReleaseHistory::starting(&starts).map_err(io::Error::other)
}

fn package_digest(generation: &Generation) -> io::Result<[u8; 32]> {
    let mut digest = [0_u8; 32];
    hex(generation.package.as_str(), &mut digest)?;
    Ok(digest)
}

fn hex(text: &str, output: &mut [u8; 32]) -> io::Result<()> {
    if text.len() != 64 {
        return Err(invalid("Package digest is not 32 bytes"));
    }
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * index..2 * index + 2], 16)
            .map_err(|_| invalid("Package digest is not hexadecimal"))?;
    }
    Ok(())
}

/// Most objects one repair visits; trust storage holds far fewer.
const REPROTECT_LIMIT: usize = 4096;

/// Protect again every object of the trust storage whose DACL only inherits again while still
/// letting in no one but this account and SYSTEM: the state folder, the trust folder and all it
/// holds, and the continuity marker. Links are never followed. Returns whether any object was
/// repaired; objects that cannot be repaired are left for the reset.
fn reprotect_harmless(installation_root: &Path, context: &JobContext) -> io::Result<bool> {
    let state =
        sugarsubstitute_configuration::layout::InstallationLayout::new(installation_root).state();
    protection::with(context, |protection| {
        let sid = protection.account_sid().to_vec();
        // Each object with whether its children are visited too.
        let mut pending = vec![
            (state.clone(), false),
            (state.join(DIRECTORY), true),
            (state.join(continuity::MARKER), false),
        ];
        let mut repaired = false;
        let mut next = 0;
        while let Some((path, descend)) = pending.get(next).cloned() {
            next += 1;
            check(context)?;
            if next > REPROTECT_LIMIT {
                break;
            }
            match crate::durable_storage::protected::security::reprotect(&path, &sid) {
                Ok(changed) => repaired |= changed,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => continue,
                Err(cause) => {
                    tracing::warn!(operation = "update_storage_kept_unprotected",
                        path = %path.display(), error = %cause, "Storage could not be protected again");
                    continue;
                }
            }
            if descend {
                for entry in Directory::open(&path, false)?.entries(context)? {
                    let entry = entry?;
                    let folder =
                        entry.file_type().is_ok_and(|kind| kind.is_dir() && !kind.is_symlink());
                    pending.push((path.join(entry.file_name()), folder));
                }
            }
        }
        Ok(repaired)
    })
}

/// Stage fresh trust for `selection` complete and ready to publish, before anything is moved.
/// Fresh trust an earlier reset left ready while the old storage stayed in place is staged again.
fn stage_fresh(
    installation_root: &Path,
    repository: [u8; 32],
    trusted_root: &[u8],
    selection: &InstallationRecord,
    context: &JobContext,
) -> Result<(), TrustFailure> {
    let _work = context.memory().claim(WORK_BYTES).map_err(io::Error::other)?;
    let trust = RepositoryTrust::bootstrap(trusted_root)?;
    let history = starting_history(selection)?;
    let (checkpoint, history) =
        initialize::fresh_records(selection.installation(), &repository, &trust, &history)?;
    let parent = Directory::open(installation_root, false)?;
    let state = ownership::state(&parent)?;
    let ready = state.path().join(READY);
    // The selection writer serializes resets; earlier stages are dead.
    protection::with(context, |protection| {
        if present(&ready, context)? {
            staging::publish(&ready, &staging::unique_path(&state, RESET_STAGE)?)?;
        }
        staging::reclaim(
            &state,
            RESET_STAGE,
            &[LOCK, releases::RECORD, RECORD],
            protection,
            context,
        )
    })?;
    let staged = initialize::InitialState::stage(
        &state,
        RESET_STAGE,
        &ready,
        &checkpoint,
        &history,
        context,
    )?;
    drop(staged.publish(&ready, context)?);
    Ok(())
}

/// Publish the fresh trust [`stage_fresh`] made ready as the installation's trust folder.
fn publish_fresh(installation_root: &Path, context: &JobContext) -> io::Result<()> {
    let state =
        sugarsubstitute_configuration::layout::InstallationLayout::new(installation_root).state();
    let ready = state.join(READY);
    let identity = Directory::open(&ready, false)?.identity()?;
    protection::with(context, |protection| {
        protection.publish(
            &ready,
            &state.join(DIRECTORY),
            identity,
            protection::Publication::Directory,
            context,
        )
    })
}

/// Whether anything is at `path`, without following a link there.
fn present(path: &Path, context: &JobContext) -> io::Result<bool> {
    match crate::durable_storage::files::inspect(path, context) {
        Ok(_) => Ok(true),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(cause) => Err(cause),
    }
}

/// Move the trust folder, its continuity marker and the update workspace into a new folder under
/// `user\` named for this reset, protected like the trust storage itself. Returns that folder,
/// or `None` when an interrupted reset already moved the trust folder and marker. The workspace
/// moves when present; it is kept in place when it cannot move.
fn set_aside(installation_root: &Path, context: &JobContext) -> io::Result<Option<PathBuf>> {
    let layout = sugarsubstitute_configuration::layout::InstallationLayout::new(installation_root);
    let state = layout.state();
    if !present(&state.join(DIRECTORY), context)?
        && !present(&state.join(continuity::MARKER), context)?
    {
        return Ok(None);
    }
    let user = Directory::open(&layout.user(), true)?;
    let (folder, destination) = protection::with(context, |protection| {
        let path = user.path().join(SET_ASIDE_FOLDER);
        protection.directory(&path, Mode::OpenOrCreate, context)?;
        let folder = Directory::open(&path, false)?;
        let destination = staging::numbered_directory(
            &folder,
            SET_ASIDE_PREFIX,
            |path| protection.directory(path, Mode::CreateNew, context).map(drop),
            context,
        )?;
        Ok((folder, destination))
    })?;
    check(context)?;
    for (source, name) in [
        (state.join(DIRECTORY), DIRECTORY),
        (state.join(continuity::MARKER), continuity::MARKER),
        (installation_root.join(WORKSPACE), WORKSPACE),
    ] {
        match staging::publish(&source, &destination.join(name)) {
            Ok(()) => {}
            Err(cause) if absent(&cause) => {}
            Err(cause) if name != WORKSPACE => return Err(cause),
            Err(cause) => {
                tracing::warn!(operation = "update_workspace_kept", error = %cause,
                    "The update workspace stays in place; trust no longer refers to it");
            }
        }
    }
    staging::retain_numbered(&folder, SET_ASIDE_PREFIX, KEPT_SET_ASIDE, &destination, context)?;
    Ok(Some(destination))
}

/// Whether a move failed only because its source does not exist.
fn absent(cause: &io::Error) -> bool {
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND};
    cause.get_ref().and_then(|cause| cause.downcast_ref::<windows::core::Error>()).is_some_and(
        |cause| {
            [ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND]
                .iter()
                .any(|code| cause.code() == code.to_hresult())
        },
    )
}

#[cfg(test)]
mod tests;
