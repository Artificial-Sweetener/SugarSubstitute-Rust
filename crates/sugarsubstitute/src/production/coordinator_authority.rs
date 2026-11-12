// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind elected entry custody to actual claims and the admitted process registry.
#[path = "coordinator_authority/admission.rs"]
mod admission;
#[path = "coordinator_authority/completion.rs"]
mod completion;
#[path = "coordinator_authority/installation.rs"]
mod installation;
#[path = "coordinator_authority/retirement.rs"]
mod retirement;
#[cfg(test)]
#[path = "coordinator_authority/tests.rs"]
mod tests;

use crate::job_sequence::JobSequence;
use std::{error::Error, fmt, path::PathBuf};
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::{
        CompletionBatch, ExecutionId, JobClass, JobCompletion, JobId, JobOutcome, JobSpec, Moment,
        Operation, OwnedJob, Scope, ScopeId, TerminalKind,
    },
    installation::FailedProcess,
    lifecycle::CoordinatorKey,
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::{
    CLAIM_OPERATION_BYTES, CLAIM_POOL_BYTES, ClaimAdmissionError, ClaimError, ClaimObservation,
    ClaimTarget, RegistryAdmissionError, RegistryError, WindowsProcessRegistry,
    WindowsResourceClaims,
    coordinator_entry::{CoordinatorEntryOwner, EntryAdmissionError, EntryPhase},
};

/// Report a rejected transition while its original typed cause remains owned here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthorityIssue {
    /// This owner cannot initialize a second claims lifetime.
    Started,
    /// Actual entry, scope or constructor identity does not match.
    Identity,
    /// Admission is permanently closed.
    Closed,
    /// Original deadline or actual provider failed.
    Clock,
    /// Actual adapter admission or native work failed.
    Adapter,
    /// The actual constructor stopped without producing a registry.
    Stopped,
    /// Partial work or a live family still needs actual settlement.
    Pending,
}
impl fmt::Display for AuthorityIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Coordinator authority transition rejected: {self:?}")
    }
}
impl Error for AuthorityIssue {}

enum Failure {
    Installation(AuthorityIssue),
    InstallationAdmission(sugarsubstitute_contracts::execution::AdmissionError),
    Entry(EntryAdmissionError),
    ClaimsAdmission(ClaimAdmissionError),
    Claims(ClaimError),
    RegistryAdmission(RegistryAdmissionError),
    Registry(RegistryError),
    Clock(sugarsubstitute_contracts::execution::ExecutionError),
    Identity(sugarsubstitute_contracts::execution::IdentityError),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CloseAttempt {
    Unissued,
    WaitingCapacity,
    AdmissionRejected,
    Admitted,
}
impl Failure {
    fn source(&self) -> &(dyn Error + 'static) {
        match self {
            Self::Installation(cause) => cause,
            Self::InstallationAdmission(cause) => cause,
            Self::Entry(cause) => cause,
            Self::ClaimsAdmission(cause) => cause,
            Self::Claims(cause) => cause,
            Self::RegistryAdmission(cause) => cause,
            Self::Registry(cause) => cause,
            Self::Clock(cause) => cause,
            Self::Identity(cause) => cause,
        }
    }
}

/// Retain actual claims, original constructor outcome and guards through checked cleanup.
/// The headless application thread owns this value. It borrows its existing scope;
/// its composing lifetime keeps that nonclone scope until the sole executor joins.
pub(crate) struct CoordinatorAuthority {
    installation: installation::Ownership,
    claims: WindowsResourceClaims,
    registry_job: Option<OwnedJob<WindowsProcessRegistry, RegistryError>>,
    registry_completion: Option<JobCompletion<WindowsProcessRegistry, RegistryError>>,
    failed_registry: Option<JobCompletion<WindowsProcessRegistry, RegistryError>>,
    constructor_failed_first: bool,
    original: EntrySetupBudget,
    association: Option<(ExecutionId, ScopeId)>,
    key: Option<CoordinatorKey>,
    initialization: Option<JobId>,
    initialization_finished: bool,
    registry_bound: bool,
    serving: bool,
    closing: bool,
    closed: bool,
    close_job: Option<(ScopeId, JobId)>,
    observations: [Option<ClaimObservation>; 32],
    observation_head: usize,
    observation_count: usize,
    close_until: Option<Moment>,
    close_attempt: CloseAttempt,
    close_notice: Option<AuthorityIssue>,
    close_notice_reported: bool,
    suppressed_failures: u64,
    failure: Option<Failure>,
    cleanup_failure: Option<Failure>,
    stopped: Option<TerminalKind>,
}
impl CoordinatorAuthority {
    /// Allocate native-free adapter state; no key, registry or serving proof exists.
    pub(crate) fn new(
        original: EntrySetupBudget,
        inherited: Option<sugarsubstitute_supervision::setup_storage::InheritedLaunch>,
    ) -> Result<Self, ClaimError> {
        Ok(Self {
            installation: installation::Ownership::new(inherited),
            claims: WindowsResourceClaims::new()?,
            registry_job: None,
            registry_completion: None,
            failed_registry: None,
            constructor_failed_first: false,
            original,
            association: None,
            key: None,
            initialization: None,
            initialization_finished: false,
            registry_bound: false,
            serving: false,
            closing: false,
            closed: false,
            close_job: None,
            observations: std::array::from_fn(|_| None),
            observation_head: 0,
            observation_count: 0,
            close_until: None,
            close_attempt: CloseAttempt::Unissued,
            close_notice: None,
            close_notice_reported: false,
            suppressed_failures: 0,
            failure: None,
            cleanup_failure: None,
            stopped: None,
        })
    }
    /// Borrow actual initialized claims; no numeric key substitutes for this owner.
    pub(crate) fn claims(&self) -> &WindowsResourceClaims {
        &self.claims
    }
    /// Mutate the same actual claims owner for coordinated resource operations.
    pub(crate) fn claims_mut(&mut self) -> &mut WindowsResourceClaims {
        &mut self.claims
    }
    /// Borrow only the registry inside its actual successful constructor completion.
    pub(crate) fn registry(&self) -> Option<&WindowsProcessRegistry> {
        match &self.registry_completion.as_ref()?.outcome {
            JobOutcome::Success(registry) => Some(registry),
            _ => None,
        }
    }
    /// Mutate actual registry custody without extracting its original memory guard.
    pub(crate) fn registry_mut(&mut self) -> Option<&mut WindowsProcessRegistry> {
        match &mut self.registry_completion.as_mut()?.outcome {
            JobOutcome::Success(registry) => Some(registry),
            _ => None,
        }
    }
    /// Borrow the same claims and actual successful registry together for adapter calls.
    pub(crate) fn owners_mut(
        &mut self,
    ) -> Option<(&mut WindowsResourceClaims, &mut WindowsProcessRegistry)> {
        match &mut self.registry_completion.as_mut()?.outcome {
            JobOutcome::Success(registry) => Some((&mut self.claims, registry)),
            _ => None,
        }
    }
    /// Return an actual consumed claims key; it alone grants no serving permission.
    pub(crate) const fn key(&self) -> Option<CoordinatorKey> {
        self.key
    }
    /// Actual claims and registry were bound to the same live elected entry owner.
    pub(crate) const fn serving(&self) -> bool {
        self.serving && !self.closing
    }
    /// Initialization or actual registry construction still owns pending work.
    pub(crate) fn pending(&self) -> bool {
        self.installation.pending()
            || self.initialization.is_some()
            || self.registry_job.is_some()
            || self.close_job.is_some()
    }
    /// Borrow the original cause without copying or erasing native/entry custody.
    pub(crate) fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        if self.constructor_failed_first {
            return match &self.registry_completion()?.outcome {
                JobOutcome::Failure(cause) => Some(cause),
                _ => None,
            };
        }
        if let Some(failure) = &self.failure {
            if matches!(failure, Failure::Installation(_)) {
                return self.installation.failure_source().or_else(|| Some(failure.source()));
            }
            return Some(failure.source());
        }
        let original = self.failed_registry.as_ref().or(self.registry_completion.as_ref())?;
        match &original.outcome {
            JobOutcome::Failure(cause) => Some(cause),
            _ => None,
        }
    }
    /// Preserve cleanup failure independently from the initial startup cause.
    pub(crate) fn cleanup_failure(&self) -> Option<&(dyn Error + 'static)> {
        self.cleanup_failure
            .as_ref()
            .map(Failure::source)
            .or_else(|| self.installation.release_failure())
    }
    /// Borrow the complete actual constructor terminal outcome, including panic and its guard.
    pub(crate) fn registry_completion(
        &self,
    ) -> Option<&JobCompletion<WindowsProcessRegistry, RegistryError>> {
        self.failed_registry.as_ref().or(self.registry_completion.as_ref())
    }
    /// Transfer resource observations preserved while draining coordinator cleanup facts.
    pub(crate) fn take_observation(&mut self) -> Option<ClaimObservation> {
        if self.observation_count == 0 {
            return None;
        }
        let value = self.observations[self.observation_head].take();
        self.observation_head = (self.observation_head + 1) % 32;
        self.observation_count -= 1;
        value
    }
    fn retain_observation(&mut self, value: ClaimObservation) {
        let at = (self.observation_head + self.observation_count) % 32;
        self.observations[at] = Some(value);
        self.observation_count += 1;
    }
    /// Retain `cause` and close. Every failure here belongs to the host itself, except storage
    /// and admission failures of installation work, which are not the release's fault.
    fn fail(&mut self, cause: Failure) {
        if !matches!(cause, Failure::Installation(_) | Failure::InstallationAdmission(_)) {
            self.observe_trial_failure(FailedProcess::Host);
        }
        if self.failure.is_none() && !self.constructor_failed_first {
            self.failure = Some(cause);
        } else if self.cleanup_failure.is_none() {
            self.cleanup_failure = Some(cause);
        } else {
            self.suppressed_failures = self.suppressed_failures.saturating_add(1);
            if self.suppressed_failures == 1 {
                tracing::error!(operation="coordinator_authority",cause=%cause.source(),
                    "Additional authority failures are counted; original causes remain retained");
            }
        }
        self.notice(AuthorityIssue::Adapter);
        self.request_close();
    }
    fn notice(&mut self, issue: AuthorityIssue) {
        if !self.close_notice_reported && self.close_notice.is_none() {
            self.close_notice = Some(issue);
        }
    }
    fn consume_close_notice(&mut self) -> Result<bool, AuthorityIssue> {
        if let Some(issue) = self.close_notice.take() {
            self.close_notice_reported = true;
            Err(issue)
        } else {
            Ok(false)
        }
    }
}
