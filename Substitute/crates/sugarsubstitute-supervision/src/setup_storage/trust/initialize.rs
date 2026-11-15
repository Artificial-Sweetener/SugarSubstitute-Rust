// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bootstrap trust by publishing a complete protected directory without replacing existing state.
use super::*;
use crate::durable_storage::write_port::FileWrite;
use std::path::PathBuf;

/// Prefix of the folder initial trust state is staged in.
const INITIAL_STAGE: &str = ".update-trust-stage";

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
        let history = sugarsubstitute_update::ReleaseHistory::empty();
        Self::establish(installation_root, repository, trust, &history, &selection, context)
    }

    /// Publish fresh trust state from `trust` and `history` for `selection`'s installation, then
    /// its continuity marker. The caller holds selection writer custody and has established that
    /// no trust state or continuity marker exists.
    pub(super) fn establish(
        installation_root: &Path,
        repository: [u8; 32],
        trust: RepositoryTrust,
        history: &sugarsubstitute_update::ReleaseHistory,
        selection: &sugarsubstitute_contracts::installation::InstallationRecord,
        context: &JobContext,
    ) -> Result<Self, TrustFailure> {
        let memory = context.memory().claim(1024 * 1024).map_err(io::Error::other)?;
        let parent = Directory::open(installation_root, false)?;
        let ownership = ownership::read(&parent)?;
        let installation = selection.installation().clone();
        let (bytes, history) = fresh_records(&installation, &repository, &trust, history)?;
        let state = ownership::state(&parent)?;
        let destination = state.path().join(DIRECTORY);
        if Self::has_state(parent.path(), context)? {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Existing update trust state prevents initialization",
            )
            .into());
        }
        // The selection writer serializes initialization; earlier attempts are dead.
        protection::with(context, |protection| {
            staging::reclaim(
                &state,
                INITIAL_STAGE,
                &[LOCK, releases::RECORD, RECORD],
                protection,
                context,
            )
        })?;
        let staged =
            InitialState::stage(&state, INITIAL_STAGE, &destination, &bytes, &history, context)?;
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
            strike_clock: crate::setup_storage::selection::StrikeClock::System,
        };
        let marker = protection::with_domain(context, |protection| {
            protection.directory(store.root.path(), Mode::Existing, context)?;
            let _writer = store.writer(protection, context)?;
            let marker = continuity::ensure(&store, protection, context)?;
            // Floors are kept outside the trust folder from the moment trust exists, so a later
            // reset either reads them or knows they were lost.
            store.carry_floors(protection, context)?;
            Ok(marker)
        })
        .map_err(TrustFailure::from)?;
        store._continuity = Some(marker);
        Ok(store)
    }

    /// Any canonical object or continuity evidence requires opening, never bootstrap fallback.
    pub(super) fn has_state(installation_root: &Path, context: &JobContext) -> io::Result<bool> {
        let state = installation_root.join(sugarsubstitute_configuration::layout::STATE);
        for name in [DIRECTORY, continuity::MARKER] {
            match crate::durable_storage::files::inspect(state.join(name), context) {
                Ok(_) => return Ok(true),
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause),
            }
        }
        Ok(false)
    }
}

/// The checkpoint and release history records of fresh trust from `trust` and `history`, bound to
/// `installation` and `repository`.
pub(super) fn fresh_records(
    installation: &sugarsubstitute_contracts::installation::InstallationId,
    repository: &[u8; 32],
    trust: &RepositoryTrust,
    history: &sugarsubstitute_update::ReleaseHistory,
) -> Result<(Vec<u8>, Vec<u8>), TrustFailure> {
    let checkpoint = envelope::encode(
        envelope::Kind::Checkpoint,
        installation,
        repository,
        &trust.checkpoint()?,
    )?;
    let history =
        envelope::encode(envelope::Kind::Releases, installation, repository, &history.encode()?)?;
    Ok((checkpoint, history))
}

/// Unpublished initialization retains its native directory identity, never authority by pathname.
pub(super) struct InitialState {
    path: PathBuf,
    identity: (u32, u32, u32),
    write: FileWrite,
}
impl InitialState {
    /// Flush all initial records in a fresh namespace named for `prefix`; abandoned staging is
    /// never reused. The namespace is one trust write: both records land, then the folder is
    /// published.
    pub(super) fn stage(
        parent: &Directory,
        prefix: &str,
        destination: &Path,
        checkpoint: &[u8],
        history: &[u8],
        context: &JobContext,
    ) -> io::Result<Self> {
        let mut write = write_port::begin(UpdateFile::Trust, &destination.join(RECORD));
        protection::with(context, |protection| {
            let path = staging::unique_path(parent, prefix)?;
            protection.directory(&path, Mode::CreateNew, context)?;
            let root = Directory::open(&path, false)?;
            files::sync(&protection.file(&path.join(LOCK), context)?, context)?;
            for (name, bytes) in [(releases::RECORD, history), (RECORD, checkpoint)] {
                let mut file = protection.file(&path.join(name), context)?;
                write.bytes(&mut file, bytes)?;
                files::sync(&file, context)?;
            }
            check(context)?;
            Ok(Self { path, identity: root.identity()?, write })
        })
    }

    /// Rename the exact completed object while retaining the selected installation's parent.
    pub(super) fn publish(
        mut self,
        destination: &Path,
        context: &JobContext,
    ) -> io::Result<Directory> {
        protection::with(context, |protection| {
            self.write.publish(|| {
                protection.publish(
                    &self.path,
                    destination,
                    self.identity,
                    protection::Publication::Directory,
                    context,
                )
            })
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
