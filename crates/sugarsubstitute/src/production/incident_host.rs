// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the genuine pre-capture controller and one bounded incident effect batch.
#[path = "incident_host/admission.rs"]
mod admission;
#[path = "incident_host/dispatch.rs"]
mod dispatch;
#[path = "incident_host/shutdown.rs"]
mod shutdown;
#[cfg(test)]
#[path = "incident_host/tests.rs"]
pub(in crate::production) mod tests;
use crate::job_sequence::JobSequence;
use std::{error::Error, fmt};
use sugarsubstitute_application::{
    incident::*,
    supervision::{FailedDesktopFact, ResourceCoordinator},
};
use sugarsubstitute_contracts::{
    execution::*,
    incident::*,
    lifecycle::{CoordinatorKey, ShutdownBudget},
};
use sugarsubstitute_execution::{BlockingInputError, ChargedBlockingInput, ExecutionClient};
use sugarsubstitute_supervision::coordinator_entry::{EntryError, VerifiedEntryNamespace};

/// One unchanged original admitted controller reservation, in bytes.
pub(in crate::production) const CONTROLLER_BYTES: u64 = 2 * 1024 * 1024;
const POLICY_BYTES: u64 = CONTROLLER_BYTES
    - size_of::<PreparationInput>() as u64
    - super::diagnostic_helper::SEED_SOURCE_BYTES
    - super::diagnostic_helper::collection::COLLECTION_SOURCE_BYTES
    - super::diagnostic_helper::reporting::REPORT_SOURCE_BYTES
    - super::diagnostic_helper::exporting::EXPORT_SOURCE_BYTES
    - super::reporter_role::parent::REPORTER_METADATA_BYTES;

/// Own the original consumed entry capability; no matching key creates it.
pub(in crate::production) struct PreparationInput {
    namespace: VerifiedEntryNamespace,
    owner: CoordinatorKey,
}
impl PreparationInput {
    /// Transfer the actual one-shot namespace proof without allocating replacement authority.
    pub(in crate::production) fn new(
        namespace: VerifiedEntryNamespace,
        owner: CoordinatorKey,
    ) -> Self {
        Self { namespace, owner }
    }
}
pub(in crate::production) struct PolicyState {
    pub(in crate::production) coordinator: IncidentCoordinator,
    pub(in crate::production) effects: [Option<IncidentEffect>; MAX_INCIDENT_EFFECTS],
}
pub(in crate::production) struct FundedState {
    pub(in crate::production) state: Box<PolicyState>,
    _claim: MemoryClaim,
}
struct FundedPolicy {
    state: Option<FundedState>,
    seed: super::diagnostic_helper::SeedService,
    collection: super::diagnostic_helper::collection::CollectionService,
    reporting: super::diagnostic_helper::reporting::ReportingService,
    exporting: super::diagnostic_helper::exporting::ExportService,
    reporter_funding: Option<super::reporter_role::parent::ReporterFunding>,
}
struct PreparedController {
    policy: FundedPolicy,
    input: ChargedBlockingInput<PreparationInput>,
}
#[derive(Debug)]
enum PreparationIdentity {
    Namespace,
    ExecutionOwner,
    Reservation,
    Deadline,
}
#[derive(Debug)]
enum PreparationCause {
    Namespace(EntryError),
    Memory(MemoryClaimError),
    Identity(PreparationIdentity),
    Capacity,
}
impl fmt::Display for PreparationCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Namespace(_) => f.write_str("Incident namespace is unavailable"),
            Self::Memory(_) | Self::Capacity => {
                f.write_str("Incident controller capacity rejected")
            }
            Self::Identity(_) => f.write_str("Incident controller identity rejected"),
        }
    }
}
impl Error for PreparationCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Namespace(cause) => Some(cause),
            Self::Memory(cause) => Some(cause),
            Self::Identity(_) | Self::Capacity => None,
        }
    }
}
struct PreparationFailure {
    cause: PreparationCause,
    _input: ChargedBlockingInput<PreparationInput>,
}
/// Observable categorical transition failure; the complete original cause remains owned here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum HostIssue {
    Started,
    Closed,
    Identity,
    Admission,
    Preparation,
    Unprepared,
    Pending,
    Decision,
}
impl fmt::Display for HostIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Incident host transition rejected: {self:?}")
    }
}
impl Error for HostIssue {}

enum HostCause {
    Admission(BlockingInputError),
    Identity(IdentityError),
    Decision(IncidentDecisionError),
}
/// Own constructor terminal delivery and the actual funded state before capture/projection.
/// Every method returns promptly on the parent composition thread.
pub(in crate::production) struct IncidentHost {
    constructor: Option<OwnedJob<PreparedController, PreparationFailure>>,
    completion: Option<JobCompletion<PreparedController, PreparationFailure>>,
    terminal: Option<TerminalKind>,
    cause: Option<HostCause>,
    closing: bool,
    shutdown: Option<ShutdownBudget>,
}
impl IncidentHost {
    /// Begin without allocation or native effects; `submit_prepare` supplies real funding.
    pub(in crate::production) const fn new() -> Self {
        Self {
            constructor: None,
            completion: None,
            terminal: None,
            cause: None,
            closing: false,
            shutdown: None,
        }
    }
    fn state(&mut self) -> Result<&mut PolicyState, HostIssue> {
        if self.closing {
            return Err(HostIssue::Closed);
        }
        match self.completion.as_mut().map(|value| &mut value.outcome) {
            Some(JobOutcome::Success(controller)) => controller
                .policy
                .state
                .as_mut()
                .map(|state| &mut *state.state)
                .ok_or(HostIssue::Pending),
            _ => Err(HostIssue::Unprepared),
        }
    }
    /// Borrow the genuine retained namespace for a closed actual operation request.
    /// No native root/path or equivalent token is reconstructed from its matching metadata.
    pub(in crate::production) fn namespace(&self) -> Result<&VerifiedEntryNamespace, HostIssue> {
        if self.closing {
            return Err(HostIssue::Closed);
        }
        match self.completion.as_ref().map(|value| &value.outcome) {
            Some(JobOutcome::Success(controller)) => Ok(&controller.input.input().namespace),
            _ => Err(HostIssue::Unprepared),
        }
    }
    /// Inspect the actual transferred terminal kind; absence is not failure or success.
    pub(in crate::production) fn preparation_outcome(&self) -> Option<TerminalKind> {
        self.completion.as_ref().map(|value| value.outcome.kind())
    }
    /// Borrow the funded service; successful seed publication still fences native launch.
    pub(in crate::production) fn seed_service(
        &mut self,
    ) -> Result<&mut super::diagnostic_helper::SeedService, HostIssue> {
        if self.closing {
            return Err(HostIssue::Closed);
        }
        match self.completion.as_mut().map(|value| &mut value.outcome) {
            Some(JobOutcome::Success(controller)) => Ok(&mut controller.policy.seed),
            _ => Err(HostIssue::Unprepared),
        }
    }
    /// Borrow the original bounded panic payload with its unchanged completion charge.
    /// This sensitive diagnostic is never automatically logged or persisted.
    pub(in crate::production) fn panic_cause(&self) -> Option<&PanicCause> {
        match self.completion.as_ref().map(|value| &value.outcome) {
            Some(JobOutcome::Panicked(cause)) => Some(cause),
            _ => None,
        }
    }
    /// Borrow original retained constructor/policy causes without detached error allocation.
    pub(in crate::production) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(cause) = self.unsubmitted_collection_failure() {
            return Some(cause);
        }
        if let Some(JobOutcome::Failure(cause)) =
            self.completion.as_ref().map(|value| &value.outcome)
        {
            return Some(&cause.cause);
        }
        match &self.cause {
            Some(HostCause::Admission(cause)) => Some(cause),
            Some(HostCause::Identity(cause)) => Some(cause),
            Some(HostCause::Decision(cause)) => Some(cause),
            None => None,
        }
    }
}

#[path = "incident_host/collection.rs"]
mod collection;
pub(in crate::production) use collection::CollectionSources;

#[path = "incident_host/exporting.rs"]
mod exporting;
#[path = "incident_host/passive_collection.rs"]
mod passive_collection;
#[path = "incident_host/reporting.rs"]
mod reporting;
#[path = "incident_host/symbols.rs"]
mod symbols;

pub(in crate::production) use symbols::AdoptedPassiveReporting;
