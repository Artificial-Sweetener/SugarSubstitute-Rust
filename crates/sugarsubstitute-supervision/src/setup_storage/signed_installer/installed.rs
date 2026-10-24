// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain an explicit installation choice through restart without treating it as accepted health.
use super::*;
use crate::setup_storage::selection::InstallationAuthorization;

/// One explicitly authorized installation, whose exact payload remains pinned until launch.
/// Authorization survives repository metadata expiry. Actual trial admission still needs live
/// run custody and the unchanged prepared selection; successful startup must establish health.
pub struct InstalledCandidate {
    candidate: Arc<PreparedCandidate>,
    authorization: InstallationAuthorization,
    _custody: CandidateCustody,
}

/// Published installation locations for display and subsequent independent reopening.
/// This value holds no setup/run custody and grants no authority to execute either path.
#[derive(Clone)]
pub struct CompletedInstallation {
    root: PathBuf,
    directory: PathBuf,
}
impl CompletedInstallation {
    /// Installation root to revalidate when the user requests Open.
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Generation completed by setup; a later application launch may have selected another one.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

impl SignedCandidate {
    /// Persist this explicit installer choice while repository evidence is still current.
    /// This does not publish shell entries, create a process, or accept application health.
    /// Automatic download workers must retain ordinary signed preparation instead.
    /// Reserve `SIGNED_TRANSFER_RESERVATION_BYTES` alongside this candidate's retained custody.
    /// # Errors
    /// Rejects stale signatures, changed selection, unsupported runtime entry and unknown records.
    /// Interrupted publication preserves the exact choice for an idempotent explicit retry.
    pub fn authorize_installation(
        self,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<InstalledCandidate, TrustFailure> {
        let authorization = self.candidate.authorize_installation(
            &self.offer.authorization(&self.trust),
            now,
            context,
        )?;
        Ok(InstalledCandidate { candidate: self.candidate, authorization, _custody: self._custody })
    }
}

impl InstalledCandidate {
    /// Publish verified root executable and shell entry, then release all setup custody.
    /// Reserve `SIGNED_TRANSFER_RESERVATION_BYTES` alongside retained candidate custody.
    /// Returning this value permits Ready while independent launchers open the installation.
    /// # Errors
    /// Preserves changed selection, unknown root files and edited shortcuts. Interrupted owned
    /// publication can be resumed by reopening this durable choice and calling this method again.
    pub fn publish(self, context: &JobContext) -> io::Result<CompletedInstallation> {
        let root = self.candidate.installation_root().to_owned();
        let directory = self.directory().to_owned();
        self.candidate.publish_installation(&self.authorization, context)?;
        Ok(CompletedInstallation { root, directory })
    }

    /// Inspect an existing prepared installation choice without HTTP or current repository metadata.
    /// Reserve `LAUNCH_RESERVATION_BYTES`. Absence returns `None`; damage is preserved as an error.
    /// Runtime files and saved choices are verified before this owner can admit any process.
    /// # Errors
    /// Rejects busy setup custody, foreign or malformed authority, unsupported engine requirements,
    /// and changed runtime/configuration. Does not accept health or acquire application run custody.
    pub fn reopen(root: &Path, context: &JobContext) -> io::Result<Option<Self>> {
        let session = InstallationSession::maintain(root, context)?;
        let Some((candidate, configuration, authorization)) =
            PreparedCandidate::reopen_installed(&session, context)?
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            candidate,
            authorization,
            _custody: CandidateCustody::Reopened {
                _session: Box::new(session),
                _configuration: Box::new(configuration),
            },
        }))
    }

    /// Exact pinned generation; the returned path alone grants no process or health authority.
    pub fn directory(&self) -> &Path {
        self.candidate.directory()
    }

    /// Retain immutable payload and exact unstarted-trial recovery after authorization is consumed.
    pub(crate) fn prepared(&self) -> &Arc<PreparedCandidate> {
        &self.candidate
    }

    /// Join durable local authorization with live run custody before publishing one trial.
    pub(crate) fn transfer(
        &self,
        context: &JobContext,
        recovery: &mut Option<UnstartedTrial>,
    ) -> io::Result<TrialTransfer> {
        if recovery.is_some() {
            return Err(invalid("Trial preparation already issued"));
        }
        let store = Arc::new(
            SelectionStore::open(self.candidate.installation_root(), context)
                .map_err(io::Error::other)?,
        );
        let mut run = InstallationRun::acquire(store, context).map_err(io::Error::other)?;
        *recovery =
            Some(self.candidate.reserve_unstarted(&run, context).map_err(io::Error::other)?);
        let admitted = Arc::clone(&self.candidate)
            .begin_installed_trial(&mut run, &self.authorization, context)
            .map_err(io::Error::other)?;
        run.transfer_candidate(admitted, context).map_err(io::Error::other)
    }
}
