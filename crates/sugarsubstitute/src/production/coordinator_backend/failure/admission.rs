// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify concrete rejection variants without traversing arbitrary error sources.
use super::*;
use sugarsubstitute_contracts::execution::AdmissionError;

impl FailureObservation {
    pub(super) fn admission(cause: &AdmissionError) -> Self {
        let stage = match cause {
            AdmissionError::ScopeClosed { .. } => "admission_scope_closed",
            AdmissionError::ShuttingDown => "admission_shutting_down",
            AdmissionError::QueueSaturated { .. } => "admission_queue_saturated",
            AdmissionError::CompletionSaturated => "admission_completion_saturated",
            AdmissionError::MemorySaturated { .. } => "admission_memory_saturated",
            AdmissionError::Memory(_) => "admission_memory",
            AdmissionError::DeadlineRequired => "admission_deadline_required",
            AdmissionError::DeadlineExpired { .. } => "admission_deadline_expired",
            AdmissionError::DuplicateScopeId { .. } => "admission_duplicate_scope",
            AdmissionError::ScopeIdentityOrder { .. } => "admission_scope_order",
            AdmissionError::ScopeCapacity => "admission_scope_capacity",
            AdmissionError::NativeScopeRequired => "admission_native_scope",
            AdmissionError::NativeReservationTooSmall { .. } => "admission_native_reservation",
            AdmissionError::DuplicateJobId { .. } => "admission_duplicate_job",
            AdmissionError::ConflictingNativeCaptureTarget(_) => "admission_conflicting_capture",
            AdmissionError::JobIdentityOrder { .. } => "admission_job_order",
            AdmissionError::ForeignScope { .. } => "admission_foreign_scope",
            AdmissionError::StaleReplacement { .. } => "admission_stale_replacement",
            AdmissionError::WrongClass { .. } => "admission_wrong_class",
            AdmissionError::InvalidReplacement { .. } => "admission_invalid_replacement",
            AdmissionError::InvalidSymbolAdmission => "admission_invalid_symbol",
            AdmissionError::InvalidReportExportAdmission => "admission_invalid_report_export",
            AdmissionError::ReportExportOutputCapacity => "admission_report_export_output_capacity",
            AdmissionError::InvalidCaptureAdmission => "admission_invalid_capture",
            AdmissionError::CaptureRetiring => "admission_capture_retiring",
            AdmissionError::NativeResources(_) => "admission_native_resources",
            AdmissionError::Unexpected(_) => "admission_unexpected",
        };
        Self::classified(FailureKind::Admission, stage)
    }

    pub(super) fn registry_admission(cause: &RegistryAdmissionError) -> Self {
        let stage = match cause {
            RegistryAdmissionError::ForeignOwner => "registry_foreign_owner",
            RegistryAdmissionError::InvalidSpecification => "registry_invalid_specification",
            RegistryAdmissionError::Closed => "registry_closed",
            RegistryAdmissionError::Saturated => "registry_saturated",
            RegistryAdmissionError::Stale => "registry_stale",
            RegistryAdmissionError::Busy => "registry_busy",
            RegistryAdmissionError::IdentityExhausted => "registry_identity_exhausted",
            RegistryAdmissionError::Execution(cause) => return Self::admission(cause),
        };
        Self::classified(FailureKind::Admission, stage)
    }

    pub(super) fn claim_admission(cause: &ClaimAdmissionError) -> Self {
        let stage = match cause {
            ClaimAdmissionError::Closed => "claim_closed",
            ClaimAdmissionError::Busy => "claim_busy",
            ClaimAdmissionError::Saturated => "claim_saturated",
            ClaimAdmissionError::Identity => "claim_identity",
            ClaimAdmissionError::Specification => "claim_specification",
            ClaimAdmissionError::Execution(cause) => return Self::admission(cause),
            ClaimAdmissionError::Registry(cause) => return Self::registry_admission(cause),
            ClaimAdmissionError::Native(cause) => {
                return Self {
                    native_code: cause.win32_code(),
                    ..Self::classified(FailureKind::Claims, cause.operation())
                };
            }
        };
        Self::classified(FailureKind::Admission, stage)
    }
}
