// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow closed retained failure facts without executing arbitrary cause callbacks.
#[path = "failure/admission.rs"]
mod admission;
use super::*;
use sugarsubstitute_execution::windows::NativeObservationError;
use sugarsubstitute_supervision::{
    ClaimAdmissionError, ClaimError, RegistryAdmissionError, RegistryError,
};
#[cfg(test)]
#[path = "failure/tests.rs"]
mod tests;
/// Closed diagnostic classes derived from the already retained concrete owner error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FailureKind {
    Source,
    Readiness,
    Campaign,
    RootExit,
    Output,
    Stopped,
    Registry,
    Native,
    Claims,
    Admission,
    Lifecycle,
    Other,
}

/// Borrow only sealed owners' static operation labels, never arbitrary cause text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FailureObservation {
    pub(super) kind: FailureKind,
    pub(super) stage: &'static str,
    pub(super) terminal: Option<TerminalKind>,
    pub(super) exit_code: Option<u32>,
    pub(super) native_operation: Option<&'static str>,
    pub(super) native_code: Option<u32>,
}
impl FailureObservation {
    /// Construct only inert closed diagnostics, never a proof or formatted cause.
    pub(super) const fn classified(kind: FailureKind, stage: &'static str) -> Self {
        Self {
            kind,
            stage,
            terminal: None,
            exit_code: None,
            native_operation: None,
            native_code: None,
        }
    }
    /// Inspect only the concrete sealed native owner, never its unknown leaf callbacks.
    fn native(kind: FailureKind, stage: &'static str, cause: &NativeObservationError) -> Self {
        Self {
            native_operation: Some(cause.stage()),
            native_code: cause.win32_code(),
            ..Self::classified(kind, stage)
        }
    }
    /// The outcome remains borrowed with its actual completion and memory charge.
    fn output(outcome: &JobOutcome<WindowsOutputReport, NativeObservationError>) -> Self {
        let observation = match outcome {
            JobOutcome::Failure(cause) => {
                Self::native(FailureKind::Output, "output_terminal", cause)
            }
            _ => Self::classified(FailureKind::Output, "output_terminal"),
        };
        Self { terminal: Some(outcome.kind()), ..observation }
    }
    /// Inspect the one original cause adopted by the authoritative resource policy.
    /// No recursive error callbacks, formatting, paths or response bytes are inspected.
    pub(super) fn retained(failure: &LifecycleFailure) -> Self {
        let Some(cause) = failure.source() else {
            return Self::classified(FailureKind::Other, "unclassified");
        };
        if let Some(source) = cause.downcast_ref::<FrontendSourceError>() {
            return Self {
                terminal: source.terminal_kind(),
                ..Self::classified(FailureKind::Source, source.stage())
            };
        }
        if let Some(transport) = cause
            .downcast_ref::<sugarsubstitute_comfy::readiness_transport::ReadinessTransportError>(
        ) {
            return Self::classified(FailureKind::Readiness, transport.stage());
        }
        if let Some(campaign) =
            cause.downcast_ref::<sugarsubstitute_comfy::readiness_campaign::CampaignError>()
        {
            use sugarsubstitute_comfy::readiness_campaign::CampaignError;
            let stage = match campaign {
                CampaignError::Transport(transport) => {
                    return Self::classified(FailureKind::Readiness, transport.stage());
                }
                CampaignError::Rejected(_) => "campaign_rejected",
                CampaignError::Budget(_) => "campaign_budget",
                CampaignError::Time(_) => "campaign_time",
                CampaignError::Clock(_) => "campaign_clock",
                CampaignError::Admission(_) => "campaign_admission",
                CampaignError::CleanupOutcome(_) => "campaign_cleanup_outcome",
                CampaignError::CleanupPanicked(_) => "campaign_cleanup_panic",
            };
            return Self::classified(FailureKind::Campaign, stage);
        }
        if let Some(exit) = cause.downcast_ref::<RootExit>() {
            return Self {
                exit_code: Some(exit.status),
                ..Self::classified(FailureKind::RootExit, "root_exit")
            };
        }
        if let Some(output) = cause.downcast_ref::<OutputTerminalFailure>() {
            return Self::output(&output.0.outcome);
        }
        if let Some(stopped) = cause.downcast_ref::<Stopped>() {
            return Self {
                terminal: Some(stopped.0),
                ..Self::classified(FailureKind::Stopped, "preparation_stopped")
            };
        }
        if let Some(registry) = cause.downcast_ref::<RegistryError>() {
            return Self {
                native_code: registry.win32_code(),
                ..Self::classified(FailureKind::Registry, registry.stage())
            };
        }
        if let Some(native) = cause.downcast_ref::<NativeObservationError>() {
            return Self::native(FailureKind::Native, native.stage(), native);
        }
        if let Some(claim) = cause.downcast_ref::<ClaimError>() {
            return Self {
                native_code: claim.win32_code(),
                ..Self::classified(FailureKind::Claims, claim.operation())
            };
        }
        if let Some(admission) = cause.downcast_ref::<ClaimAdmissionError>() {
            return Self::claim_admission(admission);
        }
        if let Some(admission) = cause.downcast_ref::<RegistryAdmissionError>() {
            return Self::registry_admission(admission);
        }
        if let Some(admission) =
            cause.downcast_ref::<sugarsubstitute_contracts::execution::AdmissionError>()
        {
            return Self::admission(admission);
        }
        if let Some(rejection) = cause.downcast_ref::<LifecycleRejection>() {
            let stage = match rejection {
                LifecycleRejection::ForeignOwner => "lifecycle_foreign_owner",
                LifecycleRejection::StaleIdentity => "lifecycle_stale_identity",
                LifecycleRejection::ProfileContention => "lifecycle_profile_contention",
                LifecycleRejection::WrongPhase => "lifecycle_wrong_phase",
                LifecycleRejection::ChangedDuplicate => "lifecycle_changed_duplicate",
                LifecycleRejection::StaleRequest => "lifecycle_stale_request",
                LifecycleRejection::Overloaded => "lifecycle_overloaded",
                LifecycleRejection::IdentityExhausted => "lifecycle_identity_exhausted",
                LifecycleRejection::InvalidClock => "lifecycle_invalid_clock",
                LifecycleRejection::EndpointMismatch => "lifecycle_endpoint_mismatch",
            };
            return Self::classified(FailureKind::Lifecycle, stage);
        }
        Self::classified(FailureKind::Other, "unclassified")
    }
}

#[derive(Debug)]
pub(super) struct Stopped(pub(super) TerminalKind);
impl std::fmt::Display for Stopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Backend preparation stopped: {:?}", self.0)
    }
}
impl Error for Stopped {}
#[derive(Debug)]
pub(super) struct RootExit {
    pub(super) process: ProcessKey,
    pub(super) status: u32,
}
impl std::fmt::Display for RootExit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Owned backend root exited: {}", self.status)
    }
}
impl Error for RootExit {}

#[derive(Debug)]
pub(super) struct OutputTerminalFailure(
    pub(super)  JobCompletion<
        WindowsOutputReport,
        sugarsubstitute_execution::windows::NativeObservationError,
    >,
);
impl std::fmt::Display for OutputTerminalFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Owned backend output stopped: {:?}", self.0.outcome.kind())
    }
}
impl Error for OutputTerminalFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.0.outcome {
            JobOutcome::Failure(cause) => Some(cause),
            _ => None,
        }
    }
}
