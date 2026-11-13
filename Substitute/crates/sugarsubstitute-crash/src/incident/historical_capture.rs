// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish one checked historical finished capture without inventing an observed process exit.
mod publication;
use super::*;
use crate::capture::CaptureRootSource;
use sugarsubstitute_contracts::{
    execution::{JobContext, MemoryClaim},
    incident::*,
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::diagnostic_storage::*;
/// Bounded outcome of actual historical source inspection, independent of M1 process policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoricalCaptureRecoveryState {
    /// Exact historical subject already has a canonical summary; it is not replaced.
    AlreadyPublished,
    /// No canonical original seed exists; source files remain untouched.
    SeedUnavailable,
    /// Actual native source availability; no Summary or process status was synthesized.
    Unavailable(CaptureEvidence),
    /// Actual payload, Summary and native disposition completed under this operation.
    Published,
}
/// Durable partial owner funded before native preparation; no retry reconstructs an old grant.
/// The caller retains it through actual terminal delivery and checked report/repository cleanup.
/// Native/C++ and escaped Summary storage retain their separate original diagnostic role loans.
pub struct HistoricalCaptureRecovery {
    pub(super) operation: Option<LogicalStoreOperationKey>,
    key: Option<IncidentKey>,
    seed: Option<IncidentSeed>,
    pub(super) report: Option<CapturedReport>,
    ticket: Option<PublishTicket>,
    pub(super) replay: Option<Box<CaptureDispositionPlan>>,
    committed: Option<IncidentCommit>,
    frame: Option<CapturedExceptionFrame>,
    frame_error: Option<SymbolFrameError>,
    state: Option<HistoricalCaptureRecoveryState>,
    retired: bool,
    _funding: MemoryClaim,
}
const _: () = assert!(
    size_of::<HistoricalCaptureRecovery>()
        + size_of::<CaptureDispositionPlan>()
        + 3 * BuildIdentity::MAX_BYTES
        + size_of::<[Option<AttachmentFacts>; MAX_ATTACHMENTS]>()
        < HistoricalCaptureRecovery::required_storage_bytes() as usize
);
impl HistoricalCaptureRecovery {
    /// Actual inline state, bounded build strings and one temporary attachment index, in bytes.
    pub const fn required_storage_bytes() -> u64 {
        4096
    }
    /// Admit real metadata backing before allocation or native effects.
    /// # Errors
    /// Rejects insufficient funding; no source or native effect has begun.
    pub fn new(
        funding: MemoryClaim,
    ) -> Result<Self, sugarsubstitute_contracts::execution::MemoryClaimError> {
        if funding.bytes() < Self::required_storage_bytes() {
            return Err(sugarsubstitute_contracts::execution::MemoryClaimError {
                requested_bytes: Self::required_storage_bytes(),
                available_bytes: funding.bytes(),
            });
        }
        Ok(Self {
            operation: None,
            key: None,
            seed: None,
            report: None,
            ticket: None,
            replay: None,
            committed: None,
            frame: None,
            frame_error: None,
            state: None,
            retired: false,
            _funding: funding,
        })
    }
    /// Observe only a completed source/publication result, not process retirement.
    pub const fn state(&self) -> Option<HistoricalCaptureRecoveryState> {
        self.state
    }
    /// Observe a bounded parsing rejection while its actual native source remains owned.
    pub const fn frame_failure(&self) -> Option<SymbolFrameError> {
        self.frame_error
    }
    /// Borrow the actual report source and independent native/clock/journal causes.
    pub const fn report(&self) -> Option<&CapturedReport> {
        self.report.as_ref()
    }
    /// Borrow a real committed Summary only after actual checked activation.
    pub const fn committed(&self) -> Option<&IncidentCommit> {
        self.committed.as_ref()
    }
    /// Move only the decoded promotion's existing allocation into partial replay custody.
    /// Other record kinds and duplicate installation preserve the original retained plan.
    /// This owns inert matching data; actual generation opening still needs checked storage.
    pub(super) fn retain_replay(
        &mut self,
        record: IncidentStoreState,
    ) -> Result<(), IncidentStoreError> {
        if self.replay.is_some() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let IncidentStoreState::CapturePromotion(plan) = record else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        self.replay = Some(plan);
        Ok(())
    }
    /// Checked-close report/token/generation before repository cleanup.
    /// # Errors
    /// Native uncertainty preserves the identical issued output and all funding.
    pub fn retire(
        &mut self,
        client: &ExecutionClient,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        if let Some(report) = &mut self.report {
            report.retire(client, context)?;
        }
        self.retired = true;
        Ok(())
    }
    /// Clear bounded Rust outputs after the caller observed terminal and repository retirement.
    /// Native retirement itself is checked here; this method grants no repository authority.
    /// # Errors
    /// Live or uncertain report owners cannot be reset or renewed.
    pub fn reset(&mut self) -> Result<(), IncidentStoreError> {
        if !self.retired {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        self.report = None;
        self.seed = None;
        self.ticket = None;
        self.replay = None;
        self.committed = None;
        self.operation = None;
        self.key = None;
        self.frame = None;
        self.frame_error = None;
        self.state = None;
        self.retired = false;
        Ok(())
    }
}
impl IncidentRepository {
    /// Recover exactly one allowed historical subject using the genuine current Root source.
    /// Existing records are preserved; old build/seed facts come only from checked protected data.
    /// All partial state is installed before effects and remains in output on failure/cancellation.
    /// # Errors
    /// Foreign origin/subject/operation, missing native evidence, changed bytes and original
    /// deadline failures preserve source custody and never create a numeric process exit.
    pub fn recover_historical_capture(
        &mut self,
        source: &CaptureRootSource,
        checked: &CheckedRecoveryCaptureOrigin,
        subject: IncidentSubject,
        observed_at: IncidentUtc,
        output: &mut HistoricalCaptureRecovery,
        context: &JobContext,
    ) -> Result<HistoricalCaptureRecoveryState, IncidentStoreError> {
        if output.operation.is_some()
            || checked.current_operation() != self.operation()
            || !sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Recovery(
                checked.origin(),
            )
            .contains(subject)?
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        source.origin_root(&self.operation, context)?;
        output.operation = Some(self.operation());
        output.key = Some(IncidentKey::new(subject));
        let result =
            self.recover_historical_capture_once(source, checked, observed_at, output, context)?;
        output.state = Some(result);
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
