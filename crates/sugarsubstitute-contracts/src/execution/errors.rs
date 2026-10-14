// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve admission decisions separately from unexpected adapter failures.

use super::{ExecutionId, Generation, JobClass, JobId, Moment, Operation, ReplacementKey, ScopeId};
use crate::crash::CaptureTarget;
use std::{error::Error, fmt};

/// Preserve an unexpected adapter failure and its original owned cause.
#[derive(Debug)]
pub struct ExecutionError {
    operation: Operation,
    cause: Box<dyn Error + Send + Sync>,
}

impl ExecutionError {
    /// Associate an original adapter cause with the operation that failed.
    pub fn new(operation: Operation, cause: Box<dyn Error + Send + Sync>) -> Self {
        Self { operation, cause }
    }

    /// Identify the failed use case without inspecting diagnostic text.
    pub const fn operation(&self) -> Operation {
        self.operation
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Execution {:?} failed: {}", self.operation, self.cause)
    }
}
impl Error for ExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

/// Reject work before its factory executes or any scheduling handle escapes.
pub enum AdmissionError {
    /// The requested scope no longer accepts work.
    ScopeClosed {
        /// Closed application scope.
        scope_id: ScopeId,
    },
    /// The executor is shutting down and admission is closed.
    ShuttingDown,
    /// The class's bounded pending queue has no available slot.
    QueueSaturated {
        /// Saturated admission class.
        class: JobClass,
    },
    /// No terminal-delivery credit remains for another admitted job.
    CompletionSaturated,
    /// The bounded retained-memory budget cannot admit this reservation.
    MemorySaturated {
        /// Requested retained-byte charge.
        requested_bytes: u64,
        /// Available retained-byte budget.
        available_bytes: u64,
    },
    /// The actual admitted reservation cannot fund independently owned input storage.
    Memory(super::MemoryClaimError),
    /// This typed operation requires an explicit original job deadline.
    DeadlineRequired,
    /// The effective deadline has already elapsed.
    DeadlineExpired {
        /// Effective ancestor/job deadline.
        deadline: Moment,
        /// Current moment in the same injected clock epoch.
        now: Moment,
    },
    /// A scope identity has already been used by this executor.
    DuplicateScopeId {
        /// Reused application scope identity.
        scope_id: ScopeId,
    },
    /// An accepted scope identity must advance the executor's bounded high-water mark.
    ScopeIdentityOrder {
        /// Latest scope identity accepted by this executor lifetime.
        latest: ScopeId,
    },
    /// The executor's fixed live/retained scope capacity has been reached.
    ScopeCapacity,
    /// Native observations require a scope admitted by the native scope boundary.
    NativeScopeRequired,
    /// A native observer's fixed state is not covered by the reservation.
    NativeReservationTooSmall {
        /// Submitted total peak reservation, in bytes.
        requested_bytes: u64,
        /// Minimum native observer state reservation, in bytes.
        minimum_bytes: u64,
    },
    /// A job identity has already been used by this executor.
    DuplicateJobId {
        /// Reused application job identity.
        job_id: JobId,
    },
    /// Another native capture reservation still holds actual custody for this full run.
    /// This conflicts across scopes until physical/source retirement and sampling settle.
    ConflictingNativeCaptureTarget(
        /// Exact coordinator, instance and generation whose custody remains held.
        CaptureTarget,
    ),
    /// An accepted job identity must advance the executor's bounded high-water mark.
    JobIdentityOrder {
        /// Latest job identity accepted by this executor lifetime.
        latest: JobId,
    },
    /// The supplied scope belongs to a different executor lifetime.
    ForeignScope {
        /// Receiving executor's identity.
        expected_owner: ExecutionId,
        /// Scope's actual executor identity.
        actual_owner: ExecutionId,
    },
    /// A surface revision does not advance its latest admitted generation.
    StaleReplacement {
        /// Requested surface generation.
        replacement: ReplacementKey,
        /// Latest generation already admitted for that surface.
        current_revision: Generation,
    },
    /// This operation cannot consume the selected admission class.
    WrongClass {
        /// Submitted use case.
        operation: Operation,
        /// Incompatible admission class.
        class: JobClass,
    },
    /// Interactive CPU work requires a replacement key; other classes prohibit it.
    InvalidReplacement {
        /// Submitted admission class.
        class: JobClass,
        /// Supplied application surface revision, if any.
        replacement: Option<ReplacementKey>,
    },
    /// A fixed symbol reservation or paired cleanup binding is invalid.
    InvalidSymbolAdmission,
    /// Original capture ledger, sole issuance or close-only custody binding is invalid.
    InvalidCaptureAdmission,
    /// The actual child has requested cleanup and no longer accepts fresh effects.
    CaptureRetiring,
    /// Native accounting does not belong to the actual original admission.
    NativeResources(super::NativeResourceError),
    /// Admission itself encountered an unexpected adapter failure.
    Unexpected(ExecutionError),
}

impl fmt::Debug for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeClosed { scope_id } => {
                formatter.debug_struct("ScopeClosed").field("scope_id", scope_id).finish()
            }
            Self::ShuttingDown => formatter.write_str("ShuttingDown"),
            Self::InvalidSymbolAdmission => formatter.write_str("InvalidSymbolAdmission"),
            Self::InvalidCaptureAdmission => formatter.write_str("InvalidCaptureAdmission"),
            Self::CaptureRetiring => formatter.write_str("CaptureRetiring"),
            Self::NativeResources(error) => {
                formatter.debug_tuple("NativeResources").field(error).finish()
            }
            Self::QueueSaturated { class } => {
                formatter.debug_struct("QueueSaturated").field("class", class).finish()
            }
            Self::CompletionSaturated => formatter.write_str("CompletionSaturated"),
            Self::MemorySaturated { requested_bytes, available_bytes } => formatter
                .debug_struct("MemorySaturated")
                .field("requested_bytes", requested_bytes)
                .field("available_bytes", available_bytes)
                .finish(),
            Self::DeadlineRequired => formatter.write_str("DeadlineRequired"),
            Self::DeadlineExpired { deadline, now } => formatter
                .debug_struct("DeadlineExpired")
                .field("deadline", deadline)
                .field("now", now)
                .finish(),
            Self::DuplicateScopeId { scope_id } => {
                formatter.debug_struct("DuplicateScopeId").field("scope_id", scope_id).finish()
            }
            Self::ScopeIdentityOrder { latest } => {
                formatter.debug_struct("ScopeIdentityOrder").field("latest", latest).finish()
            }
            Self::ScopeCapacity => formatter.write_str("ScopeCapacity"),
            Self::NativeScopeRequired => formatter.write_str("NativeScopeRequired"),
            Self::NativeReservationTooSmall { requested_bytes, minimum_bytes } => formatter
                .debug_struct("NativeReservationTooSmall")
                .field("requested_bytes", requested_bytes)
                .field("minimum_bytes", minimum_bytes)
                .finish(),
            Self::DuplicateJobId { job_id } => {
                formatter.debug_struct("DuplicateJobId").field("job_id", job_id).finish()
            }
            Self::ConflictingNativeCaptureTarget(_) => {
                formatter.write_str("ConflictingNativeCaptureTarget { redacted }")
            }
            Self::JobIdentityOrder { latest } => {
                formatter.debug_struct("JobIdentityOrder").field("latest", latest).finish()
            }
            Self::ForeignScope { expected_owner, actual_owner } => formatter
                .debug_struct("ForeignScope")
                .field("expected_owner", expected_owner)
                .field("actual_owner", actual_owner)
                .finish(),
            Self::StaleReplacement { replacement, current_revision } => formatter
                .debug_struct("StaleReplacement")
                .field("replacement", replacement)
                .field("current_revision", current_revision)
                .finish(),
            Self::WrongClass { operation, class } => formatter
                .debug_struct("WrongClass")
                .field("operation", operation)
                .field("class", class)
                .finish(),
            Self::InvalidReplacement { class, replacement } => formatter
                .debug_struct("InvalidReplacement")
                .field("class", class)
                .field("replacement", replacement)
                .finish(),
            Self::Memory(cause) => formatter.debug_tuple("Memory").field(cause).finish(),
            Self::Unexpected(value) => formatter.debug_tuple("Unexpected").field(value).finish(),
        }
    }
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeClosed { scope_id } => {
                write!(formatter, "Scope {} is closed", scope_id.get())
            }
            Self::ShuttingDown => formatter.write_str("Execution admission is shutting down"),
            Self::QueueSaturated { class } => {
                write!(formatter, "Execution queue {class:?} is saturated")
            }
            Self::CompletionSaturated => {
                formatter.write_str("Execution completion capacity is saturated")
            }
            Self::DeadlineRequired => {
                formatter.write_str("Execution operation requires an original job deadline")
            }
            Self::MemorySaturated { requested_bytes, available_bytes } => write!(
                formatter,
                "Execution reservation requires {requested_bytes} bytes with {available_bytes} available"
            ),
            Self::DeadlineExpired { deadline, now } => write!(
                formatter,
                "Execution deadline {} elapsed at {}",
                deadline.as_nanos(),
                now.as_nanos()
            ),
            Self::DuplicateScopeId { scope_id } => {
                write!(formatter, "Scope identity {} was already used", scope_id.get())
            }
            Self::ScopeIdentityOrder { latest } => {
                write!(formatter, "Scope identity must advance {}", latest.get())
            }
            Self::ScopeCapacity => formatter.write_str("Execution scope capacity is saturated"),
            Self::NativeScopeRequired => {
                formatter.write_str("Native observation requires an admitted native scope")
            }
            Self::NativeReservationTooSmall { requested_bytes, minimum_bytes } => write!(
                formatter,
                "Native observation reserves {requested_bytes} bytes but requires at least {minimum_bytes}"
            ),
            Self::DuplicateJobId { job_id } => {
                write!(formatter, "Job identity {} was already used", job_id.get())
            }
            Self::ConflictingNativeCaptureTarget(_) => {
                formatter.write_str("Native capture target custody is already reserved")
            }
            Self::JobIdentityOrder { latest } => {
                write!(formatter, "Job identity must advance {}", latest.get())
            }
            Self::ForeignScope { expected_owner, actual_owner } => write!(
                formatter,
                "Scope belongs to executor {} instead of {}",
                actual_owner.get(),
                expected_owner.get()
            ),
            Self::StaleReplacement { replacement, current_revision } => write!(
                formatter,
                "Surface {} revision {} does not advance {}",
                replacement.surface.get(),
                replacement.revision.get(),
                current_revision.get()
            ),
            Self::WrongClass { operation, class } => {
                write!(formatter, "Execution {operation:?} cannot use {class:?}")
            }
            Self::InvalidReplacement { class, replacement } => {
                write!(formatter, "Execution {class:?} has invalid replacement {replacement:?}")
            }
            Self::Memory(cause) => fmt::Display::fmt(cause, formatter),
            Self::Unexpected(error) => fmt::Display::fmt(error, formatter),
            Self::InvalidSymbolAdmission => {
                formatter.write_str("Invalid fixed symbol admission or cleanup binding")
            }
            Self::InvalidCaptureAdmission => {
                formatter.write_str("Invalid original capture admission or cleanup custody")
            }
            Self::CaptureRetiring => {
                formatter.write_str("Original capture is retiring and cannot accept fresh effects")
            }
            Self::NativeResources(error) => fmt::Display::fmt(error, formatter),
        }
    }
}
impl Error for AdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(cause) => Some(cause),
            Self::Unexpected(error) => Some(error),
            Self::NativeResources(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_run_conflict_preserves_full_matching_fact_and_redacts_diagnostics()
    -> Result<(), Box<dyn Error>> {
        use crate::lifecycle::{
            CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
        };
        let run = RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([79; 32]),
                epoch: CoordinatorEpoch::new([81; 16])?,
            },
            instance: InstanceId::new(2)?,
            generation: RunGeneration::new(3)?,
        };
        let error = AdmissionError::ConflictingNativeCaptureTarget(CaptureTarget::Desktop(run));
        assert!(
            matches!(error, AdmissionError::ConflictingNativeCaptureTarget(actual) if actual == CaptureTarget::Desktop(run))
        );
        assert_eq!(format!("{error:?}"), "ConflictingNativeCaptureTarget { redacted }");
        assert_eq!(error.to_string(), "Native capture target custody is already reserved");
        assert!(error.source().is_none());
        Ok(())
    }

    #[test]
    fn unexpected_admission_preserves_typed_original_cause() -> Result<(), Box<dyn Error>> {
        let error = AdmissionError::Unexpected(ExecutionError::new(
            Operation::Infrastructure,
            Box::new(std::io::Error::other("boundary failed")),
        ));
        let execution = error
            .source()
            .and_then(|source| source.downcast_ref::<ExecutionError>())
            .ok_or("Missing execution cause")?;
        assert_eq!(execution.operation(), Operation::Infrastructure);
        let cause = execution
            .source()
            .and_then(|source| source.downcast_ref::<std::io::Error>())
            .ok_or("Missing original cause")?;
        assert_eq!(cause.to_string(), "boundary failed");
        Ok(())
    }
}
