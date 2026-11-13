// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist update trust under independent exclusion before returning authenticated offers.
//!
//! Run on an admitted storage job. Network work happens before entry; no application lease,
//! process, network request or background task is acquired here. Missing/corrupt authoritative
//! state is an error, never permission to bootstrap again. Account protection excludes other
//! accounts; software already controlling this account is outside this storage trust boundary.
mod activation;
mod continuity;
mod discovery;
pub use activation::UpdateAuthorization;
mod envelope;
mod failure;
mod initialize;
mod installed;
mod opening;
pub(in crate::setup_storage) use installed::PublishedRelease;
mod offer_cache;
pub(crate) mod offered;
mod preparation;
pub use offer_cache::{OFFER_LOOKUP_RESERVATION_BYTES, OFFER_PUBLICATION_RESERVATION_BYTES};
pub use offered::{AuthenticatedOffer, OFFER_RESERVATION_BYTES};
pub(in crate::setup_storage) use preparation::{PreparationAttempt, try_preparation};
mod receipt;
mod records;
mod releases;
mod session;
mod subscription;
mod updater;
use super::SelectionStore;
use super::check;
use super::invalid;
use super::ownership;
use super::short_lock::LockKind;
use crate::durable_storage::{
    files::{self, Directory},
    protected::directories::Mode,
    protection::{self, Protection},
    staging,
};
pub use failure::TrustFailure;
use std::{fs::File, io, io::Write, path::Path};
use sugarsubstitute_contracts::{
    execution::{JobContext, MemoryClaim, MemoryLease},
    installation::InstallationId,
    update::Request,
};
use sugarsubstitute_update::{MetadataSet, RepositoryTrust, VerifiedOffer};

/// Protected update trust inside the installation state folder.
pub(super) const DIRECTORY: &str = "update-trust";
/// Immutable trust continuity marker beside the trust directory in the state folder.
pub(super) const CONTINUITY_MARKER: &str = "update-trust-enabled";
const RECORD: &str = "checkpoint";
const LOCK: &str = "writer";
const WORK_BYTES: u64 = 128 * 1024 * 1024;
/// Storage-job reservation for bounded metadata parsing, checkpoint copies and native custody.
pub const TRUST_RESERVATION_BYTES: u64 = WORK_BYTES + 8 * 1024 * 1024;
/// Native protection budget for later operations using an already retained trust workspace.
pub const TRUST_OPERATION_RESERVATION_BYTES: u64 = 8 * 1024 * 1024;

/// Pinned installation/repository binding; mutable trust is reloaded under exclusion every time.
/// Its original admitted reservation funds later trust parsing. Later operations use their current
/// job's cancellation, clock and native protection budget; they never reuse an old job context.
/// Simultaneous operations share the workspace bound and reject excess claims without waiting.
pub struct TrustStore {
    preparation: Option<File>,
    root: Directory,
    _installation_root: Directory,
    _ownership: File,
    _continuity: Option<File>,
    _memory: MemoryClaim,
    workspace: MemoryLease,
    cache: std::sync::Mutex<Option<super::cache::RuntimeCache>>,
    installation: InstallationId,
    repository: [u8; 32],
}
impl TrustStore {
    /// Open existing protected trust with the repository identity embedded in the distribution.
    /// All later distributions retain that identity across signing-key rotations.
    /// Complete earlier stores gain a protected continuity marker after checkpoint and history
    /// validation; their accepted verification floors remain unchanged.
    /// # Errors
    /// Rejects absent/foreign/corrupt state and unsafe filesystem ownership without resetting it.
    pub fn open(
        installation_root: &Path,
        repository: [u8; 32],
        context: &JobContext,
    ) -> Result<Self, TrustFailure> {
        Self::open_bound(installation_root, Some(repository), context)
    }

    /// Persist a validated successor root before any caller can use its authority.
    /// # Errors
    /// Preserves verification/storage causes, prior state and unpublished staging files.
    pub fn rotate(&self, root: &[u8], context: &JobContext) -> Result<(), TrustFailure> {
        self.transition(context, |trust| trust.rotate(root))
    }

    /// Save all authenticated floors, including partial progress before a later-role failure.
    /// # Errors
    /// Returns verification failure only after saving accepted progress; publication failure
    /// retains both causes and never returns authentication authority.
    pub fn refresh(
        &self,
        metadata: MetadataSet<'_>,
        now: i64,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        self.transition(context, |trust| trust.refresh(metadata, now))
    }

    /// Reauthenticate metadata and the requested descriptor, then durably save accepted floors.
    /// The returned offer permits preparation only; activation must recheck current authority.
    /// # Errors
    /// Returns no offer if metadata, routing, storage, cancellation or publication fails.
    pub fn offer(
        &self,
        metadata: MetadataSet<'_>,
        request: &Request,
        descriptor: &[u8],
        now: i64,
        context: &JobContext,
    ) -> Result<VerifiedOffer, TrustFailure> {
        self.protected(context, |protection| {
            let _writer = self.writer(protection, context)?;
            let history = self.read_releases(protection)?;
            let mut trust = self.read(protection)?;
            self.apply(&mut trust, protection, context, |trust| {
                trust.refresh(metadata, now)?;
                let offer = trust.offer(request, descriptor, now)?;
                history.check(request, &offer)?;
                Ok(offer)
            })
        })
    }

    /// Keep authority changes inside one finite local transaction and publish before return.
    fn transition<T>(
        &self,
        context: &JobContext,
        operation: impl FnOnce(&mut RepositoryTrust) -> Result<T, sugarsubstitute_update::Error>,
    ) -> Result<T, TrustFailure> {
        self.protected(context, |protection| {
            let _writer = self.writer(protection, context)?;
            let mut trust = self.read(protection)?;
            self.apply(&mut trust, protection, context, operation)
        })
    }

    /// Persist authenticated progress before returning any capability, including failed verification.
    fn apply<T>(
        &self,
        trust: &mut RepositoryTrust,
        protection: &mut Protection,
        context: &JobContext,
        operation: impl FnOnce(&mut RepositoryTrust) -> Result<T, sugarsubstitute_update::Error>,
    ) -> Result<T, TrustFailure> {
        let commit = crate::durable_storage::records::PreparedCommit::new(
            &self.root,
            RECORD,
            "pending",
            envelope::Kind::Checkpoint.limit(),
            protection,
            context,
        )?;
        check(context)?;
        let outcome = operation(trust);
        // Once verification accepts authority, cancellation must not skip its durable floor.
        // The prepared storage owner finishes this bounded commit without a cancellable open.
        let publication = (|| {
            let checkpoint = trust.checkpoint().map_err(io::Error::other)?;
            let bytes = envelope::encode(
                envelope::Kind::Checkpoint,
                &self.installation,
                &self.repository,
                &checkpoint,
            )?;
            commit.commit(&bytes)
        })();
        if let Err(storage) = publication {
            return Err(TrustFailure::Publication { storage, verification: outcome.err() });
        }
        let value = outcome?;
        check(context)?;
        Ok(value)
    }

    /// Validate security afresh while retaining directory handles through the whole operation.
    fn protected<T>(
        &self,
        context: &JobContext,
        operation: impl FnOnce(&mut Protection) -> Result<T, TrustFailure>,
    ) -> Result<T, TrustFailure> {
        check(context)?;
        let _memory = self.workspace.claim(WORK_BYTES).map_err(io::Error::other)?;
        protection::with_domain(context, |protection| {
            protection.directory(self.root.path(), Mode::Existing, context)?;
            operation(protection)
        })
        .map_err(TrustFailure::from)
    }

    /// Hold the writer exclusively for one read-modify-publish of trust state.
    /// The writer file is only an OS lock rendezvous; its existence never means busy.
    /// # Errors
    /// Waits for readers and other writers, then returns `LockTimeout` or `LockCancelled`.
    fn writer(&self, protection: &Protection, context: &JobContext) -> Result<File, TrustFailure> {
        self.locked(protection, LockKind::Exclusive, context)
    }

    /// Hold the writer shared for one consistent read of trust state; readers run together.
    fn reader(&self, protection: &Protection, context: &JobContext) -> Result<File, TrustFailure> {
        self.locked(protection, LockKind::Shared, context)
    }

    fn locked(
        &self,
        protection: &Protection,
        kind: LockKind,
        context: &JobContext,
    ) -> Result<File, TrustFailure> {
        let file = self.root.read_lockable(LOCK)?;
        protection.verify_file(&file)?;
        super::short_lock::acquire(&file, kind, context)?;
        Ok(file)
    }

    /// Retain the exact checked input through bounded reading and repository/installation binding.
    fn read(&self, protection: &Protection) -> Result<RepositoryTrust, TrustFailure> {
        let mut file = self.root.read(RECORD)?;
        protection.verify_file(&file)?;
        let bytes = files::read_bounded(&mut file, envelope::Kind::Checkpoint.limit())?;
        let checkpoint = envelope::decode(
            envelope::Kind::Checkpoint,
            &self.installation,
            &self.repository,
            &bytes,
        )?;
        Ok(RepositoryTrust::from_trusted_checkpoint(checkpoint)?)
    }

    /// Trust storage belongs to the existing selection identity, never to a path string alone.
    fn selection(
        path: &Path,
        context: &JobContext,
    ) -> Result<sugarsubstitute_contracts::installation::InstallationRecord, TrustFailure> {
        SelectionStore::open(path, context)
            .and_then(|store| store.load(context))
            .map_err(io::Error::other)?
            .ok_or_else(|| TrustFailure::Storage(invalid("Native installation selection missing")))
    }
}

#[cfg(test)]
mod tests;
