// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Detach one physically retired failed seed without resetting or discarding its original causes.
use super::*;
use sugarsubstitute_contracts::incident::{
    IncidentKey, IncidentRevision, LogicalStoreOperationKey,
};

/// Actual input correlation retained before the source consumes its one protected request.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SeedIdentity {
    operation: LogicalStoreOperationKey,
    key: IncidentKey,
    revision: IncidentRevision,
}
impl SeedIdentity {
    /// Bind the original protected request before its actual source transfer.
    pub(super) fn from_input(input: &SeedInput) -> Self {
        Self { operation: input.operation, key: input.seed.key(), revision: input.seed.revision() }
    }
}
/// Keep genuine failed/cancelled worker results and checked cleanup causes in the failed run archive.
/// The original checked cleanup reservation funds this metadata; native causes keep their own credit.
/// Holding this value grants no storage, source-removal, launch or family-settlement authority.
pub(in crate::production) struct RetiredRecoverySeed {
    failure: Option<super::super::SeedFailure>,
    completion: Option<JobCompletion<(), SeedServiceIssue>>,
    _cleanup: JobCompletion<bool, SeedServiceIssue>,
    admission: Option<AdmissionError>,
    identity: Option<IdentityError>,
    memory: Option<MemoryClaimError>,
    _claim: MemoryClaim,
    _outside: MemoryLease,
}
impl RetiredRecoverySeed {
    /// Expose actual execution panic metadata separately from scoped publication failure.
    pub(in crate::production) fn owner_failed(&self) -> bool {
        self.completion
            .as_ref()
            .is_some_and(|done| matches!(&done.outcome, JobOutcome::Panicked(_)))
    }
    /// Borrow the unchanged original failure chain; native paths and provider text stay unlogged.
    pub(in crate::production) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        self.failure
            .as_ref()
            .map(|cause| cause as &dyn Error)
            .or_else(|| {
                self.completion.as_ref().and_then(|done| match &done.outcome {
                    JobOutcome::Failure(cause) => Some(cause as &dyn Error),
                    _ => None,
                })
            })
            .or_else(|| self.admission.as_ref().map(|cause| cause as &dyn Error))
            .or_else(|| self.identity.as_ref().map(|cause| cause as &dyn Error))
            .or_else(|| self.memory.as_ref().map(|cause| cause as &dyn Error))
    }
}
impl SeedService {
    /// Match an actual installed input, original worker termination and checked file retirement.
    pub(in crate::production) fn failed_retirement_matches(
        &self,
        operation: LogicalStoreOperationKey,
        key: IncidentKey,
        revision: IncidentRevision,
    ) -> bool {
        self.installed
            && self.job.is_none()
            && self.cleanup.is_none()
            && self.input_identity == Some(SeedIdentity { operation, key, revision })
            && self.files_retired()
    }
    /// Recognize actual clean success before selecting the failed-publication retirement path.
    pub(in crate::production) fn publication_succeeded(&self) -> bool {
        self.outcome() == Some(TerminalKind::Success)
            && self.failure.is_none()
            && self.inspect(|operation| {
                operation.committed().is_some()
                    && operation.failure().is_none()
                    && operation.memory_failure().is_none()
                    && operation.retirement_failure().is_none()
            }) == Some(true)
    }
    /// Retain every original passive failure before dropping checked-closed source allocations.
    /// Repeated calls preserve the same physical proof and first metadata-allocation failure.
    pub(in crate::production) fn prepare_failed_retirement(
        &mut self,
    ) -> Result<(), SeedServiceIssue> {
        if !self.installed || self.job.is_some() || self.cleanup.is_some() || !self.files_retired()
        {
            return Err(SeedServiceIssue::Busy);
        }
        if self.publication_succeeded() {
            return Err(SeedServiceIssue::Identity);
        }
        if !self.source_retired {
            if self.failure.is_none() {
                self.failure = self.source.take_failure();
                if let (Some(failure), Some(completion)) = (&self.failure, &self.completion) {
                    failure.record(completion);
                }
            }
            self.source.release_failed_retired()?;
            self.source_retired = true;
        }
        if self.retirement_claim.is_none() {
            let memory = &self.cleanup_completion.as_ref().ok_or(SeedServiceIssue::Busy)?.memory;
            match memory.claim(size_of::<RetiredRecoverySeed>() as u64 + 1024) {
                Ok(claim) => self.retirement_claim = Some(claim),
                Err(cause) => {
                    self.retirement_memory.get_or_insert(cause);
                    return Err(SeedServiceIssue::Busy);
                }
            }
        }
        Ok(())
    }
    /// Transfer the exact retired originals only after their typed domain retirement was accepted.
    /// A rejected precondition retains the complete service for another close-only progress turn.
    pub(in crate::production) fn take_failed_retirement(
        &mut self,
    ) -> Result<RetiredRecoverySeed, SeedServiceIssue> {
        if !self.source_retired
            || !self.files_retired()
            || self.retirement_claim.is_none()
            || self.cleanup_completion.is_none()
            || self.job.is_some()
            || self.cleanup.is_some()
        {
            return Err(SeedServiceIssue::Busy);
        }
        let claim = self.retirement_claim.take().ok_or(SeedServiceIssue::Busy)?;
        let cleanup = self.cleanup_completion.take().ok_or(SeedServiceIssue::Busy)?;
        let outside = cleanup.memory.clone();
        let retired = RetiredRecoverySeed {
            failure: self.failure.take(),
            completion: self.completion.take(),
            _cleanup: cleanup,
            admission: self.admission.take(),
            identity: self.identity.take(),
            memory: self.retirement_memory.take(),
            _claim: claim,
            _outside: outside,
        };
        self.terminal = None;
        self.cleanup_terminal = None;
        self.requested = None;
        self.cleanup_binding = None;
        self.input_identity = None;
        self.source_retired = false;
        self.installed = false;
        Ok(retired)
    }
}

#[cfg(test)]
#[path = "recovery/tests.rs"]
mod tests;
