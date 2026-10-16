// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate authenticated observations and actual checked report commits.
use super::super::diagnostic_helper::reporting::ReportingService;
use super::*;

impl IncidentHost {
    /// Borrow the same funded report owner; closure permits retirement, not new work.
    pub(in crate::production) fn reporting_service(
        &mut self,
    ) -> Result<&mut ReportingService, HostIssue> {
        if self.closing {
            return Err(HostIssue::Closed);
        }
        self.retained_reporting_service()
    }
    /// Borrow existing report custody during checked shutdown without admitting replacement work.
    pub(in crate::production) fn retiring_reporting_service(
        &mut self,
    ) -> Result<&mut ReportingService, HostIssue> {
        if !self.closing {
            return Err(HostIssue::Started);
        }
        self.retained_reporting_service()
    }
    /// Locate the original funded owner without changing its admission state.
    fn retained_reporting_service(&mut self) -> Result<&mut ReportingService, HostIssue> {
        match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => Ok(&mut value.policy.reporting),
            _ => Err(HostIssue::Unprepared),
        }
    }
    /// Feed every report delivery lane once from the parent's single immutable batch.
    pub(in crate::production) fn capture_reporting(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, HostIssue> {
        let service = match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => &mut value.policy.reporting,
            _ => return Ok(false),
        };
        let work = service.capture(batch);
        let presentation = service.capture_view(batch);
        let cleanup = service.capture_cleanup(batch);
        match (work, presentation, cleanup) {
            (Ok(a), Ok(b), Ok(c)) => Ok(a || b || c),
            _ => Err(HostIssue::Identity),
        }
    }
    /// Allocate genuine readonly correlation for the exact policy-retained view.
    pub(in crate::production) fn admit_report_view(
        &mut self,
        binding: ReportBinding,
        context: IncidentRequestContext,
    ) -> Result<IncidentOperationScope, HostIssue> {
        let state = self.vacant()?;
        let result = state.coordinator.admit_report_view(binding, context);
        result.map_err(|cause| {
            self.cause.get_or_insert(HostCause::Decision(cause));
            HostIssue::Decision
        })
    }
    /// Reserve a distinct existing journal operation for actual pre-ACK recovery preparation.
    /// This leaves report mutation/presentation state intact and grants no native authority.
    pub(in crate::production) fn admit_recovery_capture_origin(
        &mut self,
        binding: ReportBinding,
        context: IncidentRequestContext,
    ) -> Result<IncidentOperationScope, HostIssue> {
        let state = self.vacant()?;
        state.coordinator.admit_recovery_capture_origin(binding, context).map_err(|cause| {
            self.cause.get_or_insert(HostCause::Decision(cause));
            HostIssue::Decision
        })
    }
    /// Observe an authenticated Presented frame after its Core validation.
    pub(in crate::production) fn observe_presented(
        &mut self,
        binding: ReportBinding,
        context: IncidentRequestContext,
    ) -> Result<(), HostIssue> {
        self.observe(IncidentServiceObservation::ReporterPresented { binding, context })
    }
    /// Retain an actual presentation failure without classifying it as acknowledgement.
    pub(in crate::production) fn observe_report_failed(
        &mut self,
        binding: ReportBinding,
        failure: ReportFailure,
        context: IncidentRequestContext,
    ) -> Result<(), HostIssue> {
        self.observe(IncidentServiceObservation::ReporterFailed { binding, failure, context })
    }
    /// Fence controls until the real same-session ViewReplaced acknowledgement.
    pub(in crate::production) fn observe_view_replaced(
        &mut self,
        binding: ReportBinding,
    ) -> Result<(), HostIssue> {
        self.observe(IncidentServiceObservation::ReportViewReplaced { binding })
    }
    /// Project only the sole desktop retirement owner's checked same-run witness.
    pub(in crate::production) fn observe_failed_family_retired(
        &mut self,
        retired: &super::super::entry_activation::retirement::RetiredDesktop,
    ) -> Result<(), HostIssue> {
        self.observe(IncidentServiceObservation::FamilySettled { run: retired.run() })
    }
    /// Project only the receiving owner's checked family, pipe and worker witness.
    pub(in crate::production) fn observe_reporter_retired(
        &mut self,
        retired: &super::super::entry_activation::RetiredReporter<'_>,
    ) -> Result<(), HostIssue> {
        self.observe(IncidentServiceObservation::ReporterRetired { binding: retired.binding() })
    }
    /// Submit only an authenticated current-view command and retain every emitted effect.
    pub(in crate::production) fn report_action(
        &mut self,
        command: ReportCommand,
        context: IncidentRequestContext,
    ) -> Result<Option<ReportBinding>, HostIssue> {
        let state = self.vacant()?;
        match state.coordinator.report_action(command, context) {
            Ok(IncidentReportDecision::CopyText { binding }) => Ok(Some(binding)),
            Ok(IncidentReportDecision::Effects(effects)) => {
                Self::retain_effects(state, effects);
                Ok(None)
            }
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Decision(cause));
                Err(HostIssue::Decision)
            }
        }
    }
    /// Adopt only the original repository's actual attempt activation after checked file cleanup.
    pub(in crate::production) fn adopt_report_attempt(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
    ) -> Result<(), HostIssue> {
        let service = self.reporting_service()?;
        if !service.files_retired() || service.outcome() != Some(TerminalKind::Success) {
            return Err(HostIssue::Pending);
        }
        let observation = service
            .inspect(|operation| {
                let proof = operation.attempt().ok_or(HostIssue::Preparation)?;
                if operation.failure().is_some()
                    || proof.operation() != scope.operation()
                    || proof.key() != scope.incident()
                    || binding.incident() != scope.incident()
                    || binding.revision() != scope.expected_revision()
                    || scope.expected_revision().next().ok() != Some(proof.revision())
                {
                    return Err(HostIssue::Identity);
                }
                Ok(IncidentServiceObservation::AttemptCommitted {
                    scope,
                    binding,
                    summary: proof.summary().clone(),
                })
            })
            .ok_or(HostIssue::Pending)??;
        self.observe(observation)
    }
    /// Adopt the constructor-private durable acknowledgement only after checked cleanup.
    pub(in crate::production) fn adopt_acknowledgement(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
    ) -> Result<(), HostIssue> {
        let service = self.reporting_service()?;
        if !service.files_retired() || service.outcome() != Some(TerminalKind::Success) {
            return Err(HostIssue::Pending);
        }
        let observation = service
            .inspect(|operation| {
                let proof = operation.acknowledgement().ok_or(HostIssue::Preparation)?;
                let committed = proof.committed();
                if operation.failure().is_some()
                    || committed.operation() != scope.operation()
                    || committed.key() != scope.incident()
                    || binding.incident() != scope.incident()
                    || binding.revision() != scope.expected_revision()
                    || scope.expected_revision().next().ok() != Some(committed.revision())
                {
                    return Err(HostIssue::Identity);
                }
                Ok(IncidentServiceObservation::Acknowledged {
                    scope,
                    binding,
                    acknowledgement: proof.acknowledgement(),
                    summary: committed.summary().clone(),
                })
            })
            .ok_or(HostIssue::Pending)??;
        self.observe(observation)
    }
}
