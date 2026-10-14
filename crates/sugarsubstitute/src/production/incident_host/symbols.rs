// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit and adopt the actual symbol-only metadata commit before report presentation.
use super::*;

impl IncidentHost {
    /// Retain the original symbol interval in the policy owner before protected work starts.
    pub(in crate::production) fn admit_report_symbols(
        &mut self,
        binding: ReportBinding,
        context: IncidentRequestContext,
    ) -> Result<IncidentOperationScope, HostIssue> {
        let state = self.vacant()?;
        state.coordinator.admit_report_symbols(binding, context).map_err(|cause| {
            self.cause.get_or_insert(HostCause::Decision(cause));
            HostIssue::Decision
        })
    }

    /// Adopt only a genuine repository activation after the original work and checked cleanup.
    /// The service keeps its sealed Symbol observation and actual completion until routing ends.
    pub(in crate::production) fn adopt_report_symbols(
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
                let proof = operation.symbols().ok_or(HostIssue::Preparation)?;
                if operation.failure().is_some()
                    || proof.operation() != scope.operation()
                    || proof.key() != scope.incident()
                    || binding.incident() != scope.incident()
                    || binding.revision() != scope.expected_revision()
                    || scope.expected_revision().next().ok() != Some(proof.revision())
                {
                    return Err(HostIssue::Identity);
                }
                Ok(IncidentServiceObservation::SymbolsCommitted {
                    scope,
                    binding,
                    summary: proof.summary().clone(),
                })
            })
            .ok_or(HostIssue::Pending)??;
        self.observe(observation)
    }
}

/// Owned policy adoption of one actual never-issued report transaction.
/// Only this Host can construct it after validating its current funded service's witness.
pub(in crate::production) struct AdoptedPassiveReporting {
    marker: super::super::diagnostic_helper::reporting::PassiveReportingRestoration,
}
impl AdoptedPassiveReporting {
    /// Authenticate the identical immutable generation marker before transferring its claim.
    /// This comparison grants no file retirement, capture or publication authority.
    pub(in crate::production) fn matches(
        &self,
        marker: super::super::diagnostic_helper::reporting::PassiveReportingRestoration,
    ) -> bool {
        self.marker == marker
    }
}
impl IncidentHost {
    /// Abandon only the current Symbols mutation proven never issued by its original service.
    /// Returned owned adoption permits a distinct generation under the same funded claim.
    /// # Errors
    /// Rejects foreign, repeated, aliased, started, uncertain or changed original custody.
    pub(in crate::production) fn adopt_report_symbols_not_written(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
        marker: super::super::diagnostic_helper::reporting::PassiveReportingRestoration,
    ) -> Result<AdoptedPassiveReporting, HostIssue> {
        let service = self.reporting_service()?;
        if !marker.is_symbols()
            || marker.operation() != scope.operation()
            || marker.binding() != binding
            || binding.incident() != scope.incident()
            || binding.revision() != scope.expected_revision()
            || marker.original().2 != scope.original().until()
            || !service.validate_passive_restoration(marker)
        {
            return Err(HostIssue::Identity);
        }
        self.observe(IncidentServiceObservation::SymbolsNotWritten { scope, binding })?;
        Ok(AdoptedPassiveReporting { marker })
    }
}
