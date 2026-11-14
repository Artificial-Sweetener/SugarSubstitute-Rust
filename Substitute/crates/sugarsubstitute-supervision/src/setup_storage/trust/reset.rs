// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Set update storage another program made untrustworthy aside and start trust again.
//!
//! Protected update trust is only as good as its security. When another account can change it,
//! its DACL inherits again, its owner changed or its label was lowered, nothing in it can be
//! relied on. The storage is moved, never deleted, into a set-aside folder under `user\` for
//! diagnosis, and fresh storage this account owns is created from the trusted root this build
//! ships. Rollback protection holds because the fresh release history starts at the release
//! that runs: nothing older than it is accepted again. The next check downloads as usual.
use super::*;
use crate::setup_storage::check;
use std::path::PathBuf;
use sugarsubstitute_contracts::{
    installation::Generation,
    update::{Channel, Component},
};
use sugarsubstitute_update::ReleaseHistory;

/// Folder under the installation's `user` folder that keeps set-aside update storage.
pub const SET_ASIDE_FOLDER: &str = "update-storage-set-aside";
/// The installation's update workspace, where downloads and extraction are staged.
const WORKSPACE: &str = ".update-workspace";

impl TrustStore {
    /// Whether `failure` means the protected update storage has security that no longer keeps
    /// other accounts out, so it must be set aside rather than retried.
    pub fn storage_untrusted(failure: &TrustFailure) -> bool {
        crate::durable_storage::protected::security::untrusted(failure)
    }

    /// Set untrusted update storage aside and create fresh trust from `trusted_root`, the root
    /// metadata this build ships. Returns the fresh store and the folder the old storage was
    /// moved to. Call only after [`Self::storage_untrusted`] judged an open of this
    /// installation's trust. The accepted release keeps starting throughout: selection is read,
    /// never changed. Run on a storage worker reserving `TRUST_RESERVATION_BYTES`.
    /// # Errors
    /// An invalid root, storage still in use by another process (retried by the next check),
    /// and storage failures. Nothing is deleted on any path.
    pub fn reset_untrusted(
        installation_root: &Path,
        repository: [u8; 32],
        trusted_root: &[u8],
        context: &JobContext,
    ) -> Result<(Self, PathBuf), TrustFailure> {
        check(context)?;
        let _work = context.memory().claim(WORK_BYTES).map_err(io::Error::other)?;
        let trust = RepositoryTrust::bootstrap(trusted_root)?;
        let store = SelectionStore::open(installation_root, context).map_err(io::Error::other)?;
        let (selection, _writer) = store.hold_writer(context).map_err(io::Error::other)?;
        let history = running_history(selection.accepted())?;
        let set_aside = set_aside(installation_root, context)?;
        let trust =
            Self::establish(installation_root, repository, trust, &history, &selection, context)?;
        tracing::warn!(operation = "update_storage_set_aside", folder = %set_aside.display(),
            "Update storage another program changed was set aside; updates were set up again");
        Ok((trust, set_aside))
    }
}

/// Release history starting every application route at the release that runs.
fn running_history(accepted: Option<&Generation>) -> io::Result<ReleaseHistory> {
    let Some(accepted) = accepted else {
        return Ok(ReleaseHistory::empty());
    };
    let platform = UpdateAuthorization::local_request(Channel::Release)?.platform;
    let mut digest = [0_u8; 32];
    hex(accepted.package.as_str(), &mut digest)?;
    let routes = [Channel::Release, Channel::Canary]
        .map(|channel| (channel, Component::Application, platform));
    ReleaseHistory::running(&routes, accepted.release.sequence(), digest).map_err(io::Error::other)
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

/// Move the trust folder, its continuity marker and the update workspace into a new folder under
/// `user\` named for this reset. The trust folder must move; the others move when present.
fn set_aside(installation_root: &Path, context: &JobContext) -> io::Result<PathBuf> {
    let layout = sugarsubstitute_configuration::layout::InstallationLayout::new(installation_root);
    let state = layout.state();
    let user = Directory::open(&layout.user(), true)?;
    let folder = Directory::open(&user.path().join(SET_ASIDE_FOLDER), true)?;
    let destination = staging::directory(&folder, "set-aside")?;
    check(context)?;
    staging::publish(&state.join(DIRECTORY), &destination.join(DIRECTORY))?;
    for (source, name) in [
        (state.join(continuity::MARKER), continuity::MARKER),
        (installation_root.join(WORKSPACE), WORKSPACE),
    ] {
        match staging::publish(&source, &destination.join(name)) {
            Ok(()) => {}
            Err(cause) if absent(&cause) => {}
            Err(cause) if name == continuity::MARKER => return Err(cause),
            Err(cause) => {
                tracing::warn!(operation = "update_workspace_kept", error = %cause,
                    "The update workspace stays in place; trust no longer refers to it");
            }
        }
    }
    Ok(destination)
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
