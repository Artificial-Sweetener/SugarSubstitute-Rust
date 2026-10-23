// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bootstrap trust by publishing a complete protected directory without replacing existing state.
use super::*;
use std::path::PathBuf;

impl TrustStore {
    /// Bootstrap exactly once while a native installation selection is still at revision zero.
    /// `trusted_root` must come from the authenticated distribution, never from the network.
    /// `repository` is the stable identifier in that distribution's repository configuration;
    /// it stays unchanged when a newer distribution supplies a rotated bootstrap root.
    /// # Errors
    /// Rejects occupied state, missing selection, progressed installations and invalid roots.
    /// Interrupted preparation leaves a separate, non-authoritative directory. A later attempt
    /// creates fresh staging; an already published trust directory is never reset or replaced.
    /// An existing continuity marker rejects initialization even when that directory is missing.
    pub fn initialize(
        installation_root: &Path,
        repository: [u8; 32],
        trusted_root: &[u8],
        context: &JobContext,
    ) -> Result<Self, TrustFailure> {
        check(context)?;
        let _work = context.memory().claim(WORK_BYTES).map_err(io::Error::other)?;
        let trust = RepositoryTrust::bootstrap(trusted_root)?;
        Self::initialize_verified(installation_root, repository, trust, context)
    }

    /// Publish a root already parsed under the caller's retained metadata memory reservation.
    pub(super) fn initialize_verified(
        installation_root: &Path,
        repository: [u8; 32],
        trust: RepositoryTrust,
        context: &JobContext,
    ) -> Result<Self, TrustFailure> {
        let selection_store =
            SelectionStore::open(installation_root, context).map_err(io::Error::other)?;
        let (selection, _selection_guard) =
            selection_store.lock_initial(context).map_err(io::Error::other)?;
        let memory = context.memory().claim(1024 * 1024).map_err(io::Error::other)?;
        let parent = Directory::open(installation_root, false)?;
        let ownership = ownership::read(&parent)?;
        let installation = selection.installation().clone();
        let bytes = envelope::encode(
            envelope::Kind::Checkpoint,
            &installation,
            &repository,
            &trust.checkpoint()?,
        )?;
        let history = envelope::encode(
            envelope::Kind::Releases,
            &installation,
            &repository,
            &sugarsubstitute_update::ReleaseHistory::empty().encode()?,
        )?;
        let destination = parent.path().join(DIRECTORY);
        if Self::has_state(parent.path())? {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Existing update trust state prevents initialization",
            )
            .into());
        }
        let staged = InitialState::stage(&parent, &bytes, &history, context)?;
        let root = staged.publish(&destination, context)?;
        let mut store = Self {
            cache: std::sync::Mutex::new(None),
            preparation: None,
            root,
            _installation_root: parent,
            _ownership: ownership,
            _continuity: None,
            _memory: memory,
            workspace: context.memory().clone(),
            installation,
            repository,
        };
        let marker = protection::with_domain(context, |protection| {
            protection.directory(store.root.path(), Mode::Existing, context)?;
            let _writer = store.writer(protection)?;
            continuity::ensure(&store, protection, context)
        })
        .map_err(TrustFailure::from)?;
        store._continuity = Some(marker);
        Ok(store)
    }

    /// Any canonical object or continuity evidence requires opening, never bootstrap fallback.
    pub(super) fn has_state(installation_root: &Path) -> io::Result<bool> {
        for name in [DIRECTORY, continuity::MARKER] {
            match std::fs::symlink_metadata(installation_root.join(name)) {
                Ok(_) => return Ok(true),
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause),
            }
        }
        Ok(false)
    }
}

/// Unpublished initialization retains its native directory identity, never authority by pathname.
struct InitialState {
    path: PathBuf,
    identity: (u32, u32, u32),
}
impl InitialState {
    /// Flush all initial records in a fresh namespace; abandoned staging is never reused.
    fn stage(
        parent: &Directory,
        checkpoint: &[u8],
        history: &[u8],
        context: &JobContext,
    ) -> io::Result<Self> {
        protection::with(context, |protection| {
            let path = staging::unique_path(parent, ".update-trust-stage")?;
            protection.directory(&path, Mode::CreateNew, context)?;
            let root = Directory::open(&path, false)?;
            protection.file(&path.join(LOCK), context)?.sync_all()?;
            for (name, bytes) in [(releases::RECORD, history), (RECORD, checkpoint)] {
                let mut file = protection.file(&path.join(name), context)?;
                file.write_all(bytes)?;
                file.sync_all()?;
            }
            check(context)?;
            Ok(Self { path, identity: root.identity()? })
        })
    }

    /// Rename the exact completed object while retaining the selected installation's parent.
    fn publish(self, destination: &Path, context: &JobContext) -> io::Result<Directory> {
        protection::with(context, |protection| {
            protection.publish(
                &self.path,
                destination,
                self.identity,
                protection::Publication::Directory,
                context,
            )
        })?;
        let root = Directory::open(destination, false)?;
        if root.identity()? != self.identity {
            return Err(invalid("Published update trust directory changed"));
        }
        check(context)?;
        Ok(root)
    }
}

#[cfg(test)]
mod tests;
