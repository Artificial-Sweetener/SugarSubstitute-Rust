// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Durably prepare one genuine recovery origin before its held carrier may run.
#[path = "origin/failure.rs"]
mod failure;
#[path = "origin/operation.rs"]
mod operation;
#[path = "origin/service.rs"]
mod service;
#[path = "origin/source.rs"]
mod source;
pub(in crate::production) use failure::{OriginFailure, OriginFailureCategory};
pub(in crate::production) use service::OriginPublication;
use std::{
    error::Error,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::{
    crash::RecoveryCaptureIdentity,
    execution::*,
    incident::LogicalStoreOperationKey,
    lifecycle::{LifecycleRejection, ShutdownBudget},
    recovery_supervisor::RecoveryGrantKey,
};
use sugarsubstitute_crash::{
    CaptureRootSource,
    incident::{IncidentRepository, IncidentStoreError, RecoveryCaptureOriginCommit},
};
use sugarsubstitute_execution::{ExecutionClient, RegisteredRecoveryGrant};
use sugarsubstitute_supervision::diagnostic_storage::{
    DIAGNOSTIC_STORAGE_BYTES, DiagnosticReadFacts, DiagnosticStorage, DiagnosticStoreOpening,
    ProtectedDiagnosticWriteRequest,
};
/// Reject a closed service transition without releasing any real pending owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum OriginIssue {
    Identity,
    Busy,
    Started,
    Admission,
    Operation,
    Cleanup,
}
impl std::fmt::Display for OriginIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recovery origin publication rejected: {self:?}")
    }
}
impl Error for OriginIssue {}
/// Preserve every original constructor input when preflight or the genuine grant rejects.
pub(in crate::production) struct OriginPublicationRejection {
    pub(in crate::production) cause: OriginConstructionFailure,
    /// The same unconsumed claim, grant, capture source and request outlive rejection reporting.
    _retained_input: OriginPublicationInput,
}
/// Original actual projection charge and native-policy inputs, before allocation.
pub(in crate::production) struct OriginPublicationInput {
    pub(in crate::production) claim: MemoryClaim,
    pub(in crate::production) grant: RegisteredRecoveryGrant,
    pub(in crate::production) capture: CaptureRootSource,
    pub(in crate::production) request: ProtectedDiagnosticWriteRequest,
    pub(in crate::production) operation: LogicalStoreOperationKey,
    pub(in crate::production) at: sugarsubstitute_contracts::incident::IncidentUtc,
}
/// Report actual source rejection separately from static storage/identity preflight.
#[derive(Debug)]
pub(in crate::production) enum OriginConstructionFailure {
    Storage,
    Identity,
    Grant(AdmissionError),
}
impl std::fmt::Display for OriginConstructionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recovery origin source construction rejected")
    }
}
impl Error for OriginConstructionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Grant(cause) => Some(cause),
            _ => None,
        }
    }
}
struct Input {
    grant: RegisteredRecoveryGrant,
    capture: CaptureRootSource,
    request: ProtectedDiagnosticWriteRequest,
    operation: LogicalStoreOperationKey,
    at: sugarsubstitute_contracts::incident::IncidentUtc,
}
/// Matching historical data copied only from the actual canonical protected read.
/// It grants no opening, capture, writer or new Root authority.
#[derive(Clone, Copy)]
struct Published {
    key: RecoveryGrantKey,
    operation: LogicalStoreOperationKey,
    origin: RecoveryCaptureIdentity,
    root_identity: [u8; 24],
    journal: DiagnosticReadFacts,
}
fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match value.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Preserve the actual live grant rejection when checking the compacted completion.
#[derive(Debug)]
pub(in crate::production) enum OriginReadyFailure {
    /// The original owner or retained completion does not match.
    State(OriginIssue),
    /// The genuine registered source rejected its current finite grant.
    Grant(AdmissionError),
}
impl std::fmt::Display for OriginReadyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recovery origin readiness rejected")
    }
}
impl Error for OriginReadyFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::State(cause) => cause,
            Self::Grant(cause) => cause,
        })
    }
}
impl std::fmt::Debug for OriginPublicationRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OriginPublicationRejection")
            .field("cause", &self.cause)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for OriginPublicationRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.cause, f)
    }
}
impl Error for OriginPublicationRejection {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

#[cfg(test)]
#[path = "origin/test_support.rs"]
mod test_support;
