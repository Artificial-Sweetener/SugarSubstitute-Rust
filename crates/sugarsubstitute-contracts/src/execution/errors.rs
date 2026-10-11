// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve admission decisions separately from unexpected adapter failures.

use super::{ExecutionId, Generation, JobClass, JobId, Moment, Operation, ReplacementKey, ScopeId};
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
#[derive(Debug)]
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
    /// Admission itself encountered an unexpected adapter failure.
    Unexpected(ExecutionError),
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
            Self::Unexpected(error) => fmt::Display::fmt(error, formatter),
        }
    }
}
impl Error for AdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unexpected(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
