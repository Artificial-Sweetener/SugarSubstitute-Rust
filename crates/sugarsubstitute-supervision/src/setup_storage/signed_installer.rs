// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose authenticated delivery and signed candidate preparation under one installation session.
use super::*;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::update::Request;
use sugarsubstitute_update::{MAX_METADATA_BYTES, MetadataSet};
mod cached;
mod distribution;
mod downloaded;
pub(crate) use cached::CachedCandidate;
pub use distribution::{InstallerDistribution, MAX_DISTRIBUTION_BYTES};

/// Storage-job budget covering trust verification, metadata, download and both source/destination
/// payload custody. Source and verified candidate files remain retained together through transfer.
/// Reserve this for import. Later jobs reuse the retained trust workspace and reserve only
/// `SIGNED_PREPARATION_RESERVATION_BYTES` or `SIGNED_TRANSFER_RESERVATION_BYTES`.
pub const SIGNED_INSTALLER_RESERVATION_BYTES: u64 = TRUST_RESERVATION_BYTES
    + OFFER_RESERVATION_BYTES
    + DOWNLOAD_RESERVATION_BYTES
    + 2 * RESERVATION_BYTES;

/// Active-job budget for preparation/staging/completion alongside retained signed-import custody.
pub const SIGNED_PREPARATION_RESERVATION_BYTES: u64 =
    RESERVATION_BYTES + TRUST_OPERATION_RESERVATION_BYTES;
/// Active-job budget for native trial transfer alongside retained signed-candidate custody.
pub const SIGNED_TRANSFER_RESERVATION_BYTES: u64 = TRUST_OPERATION_RESERVATION_BYTES;

/// Distribution-owned identity and untrusted signed inputs accompanying an embedded archive.
/// The installer must obtain `repository` and `trusted_root` from its authenticated build,
/// never from a neighboring file or network response. This owner does not authenticate an EXE.
pub struct InstallerSource<'a> {
    /// Stable repository identity, unchanged by signing-key rotation.
    pub repository: [u8; 32],
    /// Bootstrap root used only before this installation has update trust.
    pub trusted_root: &'a [u8],
    /// Ordered version/bytes pairs supplying successive roots for older installed trust.
    /// Already accepted versions are skipped; gaps and invalid signatures are errors.
    pub roots: &'a [(u32, &'a [u8])],
    /// Signed timestamp, snapshot and targets roles.
    pub metadata: MetadataSet<'a>,
    /// Exact signed channel descriptor; the subscription comes from the separate local request.
    pub descriptor: &'a [u8],
    /// Installer containing the exact signed archive interval, optionally followed by a certificate.
    pub executable: &'a Path,
}

/// Imported signed package retaining setup exclusion before provisioning or generation allocation.
pub struct SignedInstallation {
    session: InstallationSession,
    bundle: Arc<VerifiedBundle>,
    trust: TrustStore,
    offer: AuthenticatedOffer,
}
impl InstallationSession {
    /// Import a signed installer through the online updater's verifier and protected package cache.
    /// Retains this session's original exclusion. No application lease, process or network work
    /// is started. `now` supplies Unix UTC seconds; reserve `SIGNED_INSTALLER_RESERVATION_BYTES`.
    /// # Errors
    /// Rejects invalid roots, signatures, rollback, archive bytes and cancellation. Preserves
    /// accepted signing floors, installation data and partial cache bytes for a subsequent attempt.
    pub fn import_installer(
        self,
        source: InstallerSource<'_>,
        request: Request,
        workspace: &Path,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<SignedInstallation, TrustFailure> {
        check(context)?;
        UpdateAuthorization::require_application_request(&request)?;
        if source.roots.len() > 1024
            || source.roots.iter().any(|(_, bytes)| bytes.len() > MAX_METADATA_BYTES)
            || source.roots.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err(sugarsubstitute_update::Error::Bound.into());
        }
        let trust = self.update_trust(source.repository, source.trusted_root, context)?;
        for (version, root) in source.roots {
            let next = trust.next_root_version(context)?;
            if *version < next {
                continue;
            }
            if *version != next {
                return Err(invalid("Installer signing-root history has a gap").into());
            }
            trust.rotate(root, context)?;
        }
        let offer = AuthenticatedOffer::authenticate(
            &trust,
            request,
            source.metadata,
            source.descriptor,
            now(),
            context,
        )?;
        let authorization = offer.authorization(&trust);
        let target = trust
            .offer(
                authorization.metadata,
                authorization.request,
                authorization.descriptor,
                now(),
                context,
            )?
            .into_package()?;
        let download = Download::import_installer(
            self.directory(),
            target,
            source.executable,
            &mut now,
            context,
        )
        .map_err(io::Error::other)?;
        self.import_download(trust, offer, download, workspace, now, context)
    }
}
impl SignedInstallation {
    /// Verified provisioning executable and package directory, retained by this owner.
    pub fn worker_paths(&self) -> (PathBuf, PathBuf) {
        self.bundle.worker_paths()
    }

    /// Journal a fresh generation before allocation, resume an exact pending release, or
    /// supersede an obsolete unlaunched release while preserving its payloads and accepted state.
    /// Rechecks current signed authority using `now` in Unix UTC seconds before reservation.
    /// Reserve `SIGNED_PREPARATION_RESERVATION_BYTES` for this worker.
    /// # Errors
    /// Rejects competing owners, live trials and unauthenticated replacements.
    pub fn prepare(
        self,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<SignedPreparation, TrustFailure> {
        let prepared = self.session.prepare_signed(
            self.bundle,
            &self.offer.authorization(&self.trust),
            now,
            context,
        )?;
        Ok(SignedPreparation { prepared, trust: self.trust, offer: self.offer })
    }
}

/// Signed preparation retains original setup exclusion and exact reauthorization inputs.
pub struct SignedPreparation {
    prepared: PreparedInstallation,
    trust: TrustStore,
    offer: AuthenticatedOffer,
}
impl SignedPreparation {
    /// Saved runtime for update/repair; its absence means fresh provisioning is required.
    pub fn maintenance_runtime(&self) -> Option<&RuntimePaths> {
        self.prepared.maintenance_runtime()
    }

    /// Stage verified desktop bytes independently of ComfyUI provisioning.
    /// Reserve `SIGNED_PREPARATION_RESERVATION_BYTES` for this worker.
    /// # Errors
    /// Preserves cancellation and storage errors and leaves partial files for recovery.
    pub fn stage(&self, context: &JobContext) -> io::Result<()> {
        self.prepared.stage(context)
    }

    /// Complete a candidate using the same signed request and exact archive that were imported.
    /// `now` supplies Unix UTC seconds. This operation neither launches nor accepts the candidate.
    /// Reserve `SIGNED_PREPARATION_RESERVATION_BYTES`; trust parsing uses retained import capacity.
    /// # Errors
    /// Rechecks current signing/release floors, expiry, saved configuration and package custody.
    pub fn complete(
        self,
        runtime: &RuntimePaths,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<SignedCandidate, TrustFailure> {
        let candidate = self.prepared.prepare_update(
            runtime,
            &self.offer.authorization(&self.trust),
            now,
            context,
        )?;
        Ok(SignedCandidate {
            candidate,
            trust: self.trust,
            offer: self.offer,
            _custody: CandidateCustody::Imported { _prepared: Box::new(self.prepared) },
        })
    }
}

/// Completed candidate with setup custody retained until launch admission or disposal.
pub struct SignedCandidate {
    candidate: Arc<PreparedCandidate>,
    trust: TrustStore,
    offer: AuthenticatedOffer,
    _custody: CandidateCustody,
}

/// Keep installation mutation and saved configuration custody through either delivery path.
enum CandidateCustody {
    Imported {
        _prepared: Box<PreparedInstallation>,
    },
    Reopened {
        _session: Box<InstallationSession>,
        _configuration: Box<super::existing::ExistingInstallation>,
    },
}
impl SignedCandidate {
    /// Save restart evidence only after this exact candidate has completed preparation.
    /// A rejected competing package must not replace the existing candidate's cached offer.
    /// Reserve offer-cache capacity alongside retained preparation and trust-operation capacity.
    pub(crate) fn save_restart_evidence(&self, context: &JobContext) -> Result<(), TrustFailure> {
        self.trust.remember_offer(&self.offer, context)
    }

    /// Exact verified candidate directory; no health or accepted-selection claim is implied.
    pub fn directory(&self) -> &Path {
        self.candidate.directory()
    }

    /// Restore an unstarted creator's exact revision while retaining pinned files and current trust.
    pub(crate) fn restore_unstarted(
        &self,
        recovery: &UnstartedTrial,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        self.candidate.restore_unstarted(recovery, context)
    }

    /// Reauthorize this candidate under independent run custody and prepare explicit handle transfer.
    /// The native creator retains `recovery` before admission and through command construction.
    /// No process starts here. Retain the returned owner through actual native process creation.
    /// Borrows preparation so busy run ownership or rejected execution admission does not discard
    /// the candidate. Repeated calls still require its exact prepared selection and current trust.
    /// Reserve `SIGNED_TRANSFER_RESERVATION_BYTES` for this worker.
    /// # Errors
    /// Rejects busy application ownership, expired authority and changed selection. Existing trial
    /// state after a transfer failure is recovered using the retained original creator custody.
    pub(crate) fn transfer(
        &self,
        now: impl FnMut() -> i64,
        context: &JobContext,
        recovery: &mut Option<UnstartedTrial>,
    ) -> Result<TrialTransfer, TrustFailure> {
        if recovery.is_some() {
            return Err(invalid("Trial preparation already issued").into());
        }
        let store = Arc::new(
            SelectionStore::open(self.candidate.installation_root(), context)
                .map_err(io::Error::other)?,
        );
        let mut run = InstallationRun::acquire(store, context).map_err(io::Error::other)?;
        *recovery =
            Some(self.candidate.reserve_unstarted(&run, context).map_err(io::Error::other)?);
        let admitted = Arc::clone(&self.candidate).begin_trial(
            &mut run,
            &self.offer.authorization(&self.trust),
            now,
            context,
        )?;
        Ok(run.transfer_candidate(admitted, context).map_err(io::Error::other)?)
    }
}

#[cfg(test)]
mod tests;
