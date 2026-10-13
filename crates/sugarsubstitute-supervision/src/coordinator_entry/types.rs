// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report entry owner state and causes without exposing native custody.
use crate::ClaimError;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    configuration::ConfigurationError,
    coordinator_entry::EntryOwnerDegradation,
    execution::{AdmissionError, JobId, TerminalKind},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId},
};
/// Reject inert entry input before any native effect.
#[derive(Debug)]
pub enum EntryInputError {
    /// The authoritative configuration owner rejected an original path.
    Configuration(ConfigurationError),
    /// Original path allocations exceed the bounded input allowance.
    Capacity,
}
impl fmt::Display for EntryInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Coordinator entry input rejected")
    }
}
impl Error for EntryInputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Configuration(cause) => Some(cause),
            Self::Capacity => None,
        }
    }
}
/// Reject entry admission without changing existing native custody.
#[derive(Debug)]
pub enum EntryAdmissionError {
    /// The entry owner has closed admission.
    Closed,
    /// An actual retained finite operation is still outstanding.
    Busy,
    /// The caller supplied a foreign execution, claim or peer identity.
    Identity,
    /// The requested finite operation or original deadline is invalid.
    Specification,
    /// Preserve genuine original preparation control with unchanged custody.
    Preparation(sugarsubstitute_execution::EntryPreparationError),
    /// Preserve actual prepared transport conversion and bounded original post causes.
    PreparedTransport(sugarsubstitute_execution::EntryTransportError),
    /// Actual execution admission rejected the operation.
    Execution(AdmissionError),
    /// Sampling the actual execution clock failed with its original cause.
    Clock(sugarsubstitute_contracts::execution::ExecutionError),
    /// Actual cached native proof validation failed.
    Native(ClaimError),
    /// The authoritative claims owner rejected the retained binding.
    Claims(crate::ClaimAdmissionError),
}
impl fmt::Display for EntryAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Coordinator entry admission rejected")
    }
}
impl Error for EntryAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Execution(cause) => Some(cause),
            Self::Preparation(cause) => Some(cause),
            Self::PreparedTransport(cause) => Some(cause),
            Self::Clock(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            Self::Claims(cause) => Some(cause),
            _ => None,
        }
    }
}
/// Preserve actual native or completion-contract failure.
#[derive(Clone, Debug)]
pub struct EntryError(pub(crate) ClaimError);
impl fmt::Display for EntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Coordinator entry operation failed")
    }
}
impl Error for EntryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}
/// Cached owner phase; it does not grant native admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryPhase {
    /// No finite discovery has been admitted.
    Inert,
    /// Actual discovery is outstanding.
    Discovering,
    /// Actual namespace and installed image were retained.
    Discovered,
    /// Actual election acquisition is outstanding.
    Electing,
    /// Fresh retained entry election permits claims preparation.
    ElectionHeld,
    /// A validated record supplies a candidate for actual authenticated transport.
    Candidate,
    /// A contended exclusion requires independently verified endpoint discovery.
    Contended,
    /// Actual retained claims bind the current coordinator.
    Serving,
    /// Admission closed; native obligations remain independent.
    Closing,
    /// Actual checked native retirement and finite-job consumption completed.
    Settled,
}
/// Observe immutable cached entry metadata without native access or mutex locking.
#[derive(Clone, Copy, Debug)]
pub struct EntrySnapshot {
    /// Current cached phase.
    pub phase: EntryPhase,
    /// Actual discovered namespace, if successfully consumed.
    pub namespace: Option<NamespaceId>,
    /// Actual current bound coordinator, if present.
    pub owner: Option<CoordinatorKey>,
    /// Exact outstanding finite operation.
    pub pending: Option<JobId>,
}
/// Transfer causal facts only after exact owned finite-job consumption.
#[derive(Clone, Debug)]
pub enum EntryObservation {
    /// Actual namespace and installed running image were retained successfully.
    NamespacePrepared(NamespaceId),
    /// Actual fresh election and source-closed Missing/Empty observation are retained.
    ElectionHeld,
    /// A valid record supplies only a candidate epoch, awaiting native peer proof.
    Candidate(CoordinatorEpoch),
    /// Actual stable exclusion contention supplies no owner authority.
    Contended,
    /// Unknown ownership remains preserved.
    Degraded(EntryOwnerDegradation),
    /// Original unexpected cause; unresolved native custody remains owned.
    Failed(EntryError),
    /// Actual finite terminal outcome, independent of native acquisition or closure.
    Stopped(TerminalKind),
    /// Actual namespace/election/path/token closure and job consumption completed.
    Retired,
}
