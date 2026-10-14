// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the original report effect through genuine symbol work and independent checked cleanup.
use super::*;
use sugarsubstitute_crash::incident::{
    SymbolReportFailure, SymbolReportObservation, SymbolReportRequest, SymbolReportResult,
    SymbolReportService,
};
use sugarsubstitute_supervision::committed_reader::ProtectedCommittedReadRequest;
#[path = "symbols/advance.rs"]
mod advance;
#[path = "symbols/metadata.rs"]
mod metadata;
#[path = "symbols/passive.rs"]
mod passive;
#[path = "symbols/receiving.rs"]
mod receiving;
#[path = "symbols/retirement.rs"]
mod retirement;
#[cfg(test)]
#[path = "symbols/tests.rs"]
mod tests;

/// Same protected request, sealed native result and CAS scope while the parent retains its Launch.
/// No copied incident identity can construct a provider observation or terminal memory lease.
pub(super) struct SymbolReceiving {
    binding: ReportBinding,
    read_scope: IncidentOperationScope,
    request: Option<SymbolReportRequest>,
    service: Option<SymbolReportService>,
    observation: Option<SymbolReportObservation>,
    failure: Option<SymbolReportFailure>,
    first: Option<SymbolReceivingCause>,
    admission: Option<AdmissionError>,
    cleanup_admission: Option<AdmissionError>,
    submitted: bool,
    cancelled: bool,
    delivered: bool,
    metadata_scope: Option<IncidentOperationScope>,
    metadata_transferred: bool,
    metadata_retired: bool,
    ready: bool,
    closing: bool,
    runtime: Option<(ExecutionId, ScopeId)>,
    adopted_passive: Option<crate::production::incident_host::AdoptedPassiveReporting>,
    passive: Option<RetainedPassiveReporting>,
}
enum SymbolReceivingCause {
    Admission(AdmissionError),
    Identity(IdentityError),
    State,
}
impl std::fmt::Debug for SymbolReceivingCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SymbolReceivingCause")
    }
}
impl std::fmt::Display for SymbolReceivingCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Actual symbol receiving failed")
    }
}
impl Error for SymbolReceivingCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Admission(cause) => Some(cause),
            Self::Identity(cause) => Some(cause),
            Self::State => None,
        }
    }
}
impl SymbolReceiving {
    /// Bind the parent's same retained effect to its genuine cached namespace borrowing request.
    /// The read scope is the policy owner's existing current-report read admission.
    /// # Errors
    /// Rejects an unrelated operation, binding, unavailable dump or previously resolved summary.
    pub(super) fn new(
        original: &Option<IncidentEffect>,
        read_scope: IncidentOperationScope,
        protected: &mut Option<ProtectedCommittedReadRequest>,
    ) -> Result<Self> {
        let Some(IncidentEffect::LaunchReporter { binding, summary }) = original.as_ref() else {
            return Err("Actual symbol source is not a queued report".into());
        };
        let request = protected.as_ref().ok_or("Actual protected Symbol read missing")?;
        let CaptureEvidence::Finished(dump) = summary.facts().capture() else {
            return Err("Actual committed report has no finished dump".into());
        };
        if request.operation() != read_scope.operation()
            || binding.incident() != read_scope.incident()
            || binding.revision() != read_scope.expected_revision()
            || summary.seed().key() != binding.incident()
            || summary.revision() != binding.revision()
            || matches!(summary.facts().symbols(), SymbolOutcome::Resolved(_))
        {
            return Err("Original symbol read correlation changed".into());
        }
        let (key, revision) = (binding.incident(), binding.revision());
        let request = SymbolReportRequest::new(
            protected.take().ok_or("Actual protected Symbol request already consumed")?,
            key,
            revision,
            dump,
        );
        Ok(Self {
            binding: *binding,
            read_scope,
            request: Some(request),
            service: None,
            observation: None,
            failure: None,
            first: None,
            admission: None,
            cleanup_admission: None,
            submitted: false,
            cancelled: false,
            delivered: false,
            metadata_scope: None,
            metadata_transferred: false,
            metadata_retired: false,
            ready: false,
            closing: false,
            runtime: None,
            adopted_passive: None,
            passive: None,
        })
    }
    /// Feed the same genuine global batch once; this owner never drains or waits for execution.
    pub(super) fn capture(&mut self, batch: &CompletionBatch) {
        if let Some(service) = &mut self.service {
            service.capture(batch);
        }
    }
    /// Retain the original worker/provider/retirement capsule and exact admission cause.
    pub(super) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        self.first
            .as_ref()
            .map(|cause| cause as _)
            .or_else(|| self.failure.as_ref().map(|cause| cause as _))
            .or_else(|| self.cleanup_admission.as_ref().map(|cause| cause as _))
            .or_else(|| self.passive.as_ref().and_then(RetainedPassiveReporting::failure))
            .or_else(|| self.admission_failure().map(|cause| cause as _))
    }
    /// Borrow the original categorical overload rejection without treating it as a terminal.
    pub(super) fn admission_failure(&self) -> Option<&AdmissionError> {
        self.admission.as_ref()
    }
    /// Read actual result delivery independently from physical settlement of its failure capsule.
    pub(super) const fn result_delivered(&self) -> bool {
        self.delivered
    }
    /// Distinguish an actual failed result from retained transient admission diagnostics.
    pub(super) fn native_failed(&self) -> bool {
        self.first.is_some() || self.failure.is_some()
    }
    /// Borrow the original unchanged first cutoff; metadata cannot renew native symbol work.
    pub(super) fn until(&self) -> Moment {
        self.read_scope.original().until()
    }
    /// True only after actual native result settlement and any issued metadata checked cleanup.
    /// A failure capsule with uncertain physical retirement never becomes settled here.
    pub(super) fn settled(&self) -> bool {
        self.native_settled()
            && (!self.metadata_transferred || self.metadata_retired)
            && (self.ready || self.closing)
    }
    fn native_settled(&self) -> bool {
        match &self.failure {
            Some(failure) => self.delivered && failure.settled(),
            None => {
                self.delivered || (self.first.is_some() || self.closing) && self.service.is_none()
            }
        }
    }
    /// Permit the parent to borrow or move its same retained whole Launch only after real readiness.
    pub(super) const fn ready(&self) -> bool {
        self.ready
    }
}
