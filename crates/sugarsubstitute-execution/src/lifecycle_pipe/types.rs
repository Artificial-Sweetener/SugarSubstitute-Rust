// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep transport observations distinct from authenticated application commands.
use crate::native::NativeObservationError;
use std::{fmt, num::NonZeroU64};
use sugarsubstitute_contracts::lifecycle::{ProcessKey, RunKey};

/// Maximum complete lifecycle message, including its versioned protocol header.
pub const MAX_LIFECYCLE_MESSAGE_BYTES: usize = 65_536;
/// Admitted process storage for two message buffers and fixed native/security owners.
pub const MIN_LIFECYCLE_PIPE_BYTES: u64 = 192 * 1024;
/// Positive connection-local operation identity; exhaustion never wraps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecyclePipeOperationId(NonZeroU64);
impl LifecyclePipeOperationId {
    /// Reject zero before publishing a command or invoking a native operation.
    pub fn new(value: u64) -> Result<Self, LifecyclePipeError> {
        NonZeroU64::new(value).map(Self).ok_or(LifecyclePipeError::InvalidOperation)
    }
    /// Return the identity as a fact, without native authority.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
/// Reject a command without silently overwriting another admitted operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecyclePipeError {
    /// The session is stopping or its terminal record is no longer current.
    Closed,
    /// The fixed operation, buffer or observation slot is occupied.
    Busy,
    /// Authentication/connection preparation is not yet established.
    NotReady,
    /// A zero, reused or decreasing operation identity was supplied.
    InvalidOperation,
    /// The original command deadline is already expired.
    DeadlineExpired,
    /// A command attempted to extend the original admitted handshake.
    InvalidDeadline,
    /// The complete message is empty or exceeds the fixed message bound.
    InvalidMessage,
    /// This single-transfer native artifact was already taken.
    AlreadyTransferred,
}
impl fmt::Display for LifecyclePipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Lifecycle pipe command rejected: {self:?}")
    }
}
impl std::error::Error for LifecyclePipeError {}
/// Preserve one original Windows QPC deadline across parent/child process epochs.
/// These clock facts grant neither authentication nor native access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecyclePipeDeadline {
    pub(crate) ticks: i64,
    pub(crate) frequency: i64,
}
impl LifecyclePipeDeadline {
    /// Validate positive signed Windows counter facts before conversion or admission.
    pub fn new(ticks: i64, frequency: i64) -> Result<Self, LifecyclePipeError> {
        if ticks <= 0 || frequency <= 0 {
            return Err(LifecyclePipeError::InvalidDeadline);
        }
        Ok(Self { ticks, frequency })
    }
    /// Return the unchanged absolute QPC deadline ticks.
    pub const fn ticks(self) -> i64 {
        self.ticks
    }
    /// Return the exact positive QPC ticks per second.
    pub const fn frequency(self) -> i64 {
        self.frequency
    }
}
/// Own the child's bootstrap capability for the native protocol encoder only.
/// Never send this value to page JavaScript, backend environments or logging.
pub struct LifecycleClientCapability(pub(crate) [u8; 32]);
impl LifecycleClientCapability {
    /// Borrow the exact Hello capability; this value alone is not authentication proof.
    pub const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl fmt::Debug for LifecycleClientCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LifecycleClientCapability { redacted }")
    }
}
impl Drop for LifecycleClientCapability {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
/// Kernel-verified connection facts; construction is private to the transport owner.
/// Facts grant no termination, profile, resource or UI-loop authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedLifecyclePeer {
    pub(crate) run: RunKey,
    pub(crate) process: Option<ProcessKey>,
    pub(crate) pid: u32,
    pub(crate) birth: u64,
    pub(crate) session: u32,
}
impl VerifiedLifecyclePeer {
    /// Return the exact admitted run whose bootstrap and native peer were verified.
    pub const fn run(self) -> RunKey {
        self.run
    }
    /// Return the enrolled owned desktop registry key on the server side.
    pub const fn process(self) -> Option<ProcessKey> {
        self.process
    }
    /// Return kernel PID metadata, never process-control authority.
    pub const fn process_id(self) -> u32 {
        self.pid
    }
    /// Return kernel creation time in100ns units since1601.
    pub const fn creation_time_100ns(self) -> u64 {
        self.birth
    }
    /// Return the verified Windows session identity.
    pub const fn session_id(self) -> u32 {
        self.session
    }
}
/// Transfer one bounded actual transport fact, separately from terminal delivery.
pub enum LifecyclePipeObservation {
    /// Server pipe and immutable bootstrap section actually exist.
    Prepared,
    /// Actual pipe connection exists; application authentication is still pending.
    Connected,
    /// Kernel identity and per-run capability were verified on this connection.
    Authenticated(VerifiedLifecyclePeer),
    /// An issued native read remains pending; this is not queue admission proof.
    ReadPending {
        /// Exact read whose retained OVERLAPPED/buffer must settle before terminal.
        operation: LifecyclePipeOperationId,
    },
    /// One complete message returned; the guard retains its charged buffer.
    Received {
        /// Exact admitted connection-local read identity.
        operation: LifecyclePipeOperationId,
        /// Complete bytes and their retained real memory claim.
        frame: super::ReceivedLifecycleFrame,
    },
    /// Actual native write acceptance, sampled immediately before issue in the local epoch.
    /// This historical fact does not assert completion or current application eligibility.
    WriteIssued {
        /// Exact admitted write identity.
        operation: LifecyclePipeOperationId,
        /// Execution-owner local clock sample at native issue.
        at: sugarsubstitute_contracts::execution::Moment,
    },
    /// The write was vetoed before native issue and its prepared callback quiesced.
    WriteRevoked {
        /// Exact revoked write identity.
        operation: LifecyclePipeOperationId,
        /// Original charged allocation; dropping returns it to this session.
        buffer: super::LifecyclePipeWriteBuffer,
    },
    /// The original write actually completed with this byte count and callback quiescence.
    /// This effect may be observed during retirement after a stop; it does not assert
    /// on-time success or peer processing, and never changes the original terminal outcome.
    Written {
        /// Exact admitted connection-local write identity.
        operation: LifecyclePipeOperationId,
        /// Actual complete native byte count.
        bytes: u32,
    },
}
impl fmt::Debug for LifecyclePipeObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prepared => f.write_str("Prepared"),
            Self::Connected => f.write_str("Connected"),
            Self::Authenticated(peer) => f.debug_tuple("Authenticated").field(peer).finish(),
            Self::ReadPending { operation } => {
                f.debug_struct("ReadPending").field("operation", operation).finish()
            }
            Self::Received { operation, frame } => f
                .debug_struct("Received")
                .field("operation", operation)
                .field("bytes", &frame.as_bytes().len())
                .finish(),
            Self::WriteIssued { operation, at } => {
                f.debug_struct("WriteIssued").field("operation", operation).field("at", at).finish()
            }
            Self::WriteRevoked { operation, .. } => {
                f.debug_struct("WriteRevoked").field("operation", operation).finish_non_exhaustive()
            }
            Self::Written { operation, bytes } => f
                .debug_struct("Written")
                .field("operation", operation)
                .field("bytes", bytes)
                .finish(),
        }
    }
}
/// Report checked transport retirement; this never proves process/family settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecyclePipeReport {
    /// The admitted run, if a child bootstrap was successfully validated.
    pub run: Option<RunKey>,
    /// Number of completely observed messages, without claiming semantic effects.
    pub received_messages: u64,
    /// Number of completely observed writes, without claiming peer processing.
    pub completed_writes: u64,
}
/// Preserve the original native cause and fatal thread-identity restoration failure.
#[derive(Clone)]
pub struct LifecyclePipeFailure {
    pub(crate) cause: NativeObservationError,
    pub(crate) identity_unrestored: bool,
}
impl LifecyclePipeFailure {
    /// Preserve an ordinary native failure without declaring fatal identity contamination.
    pub(crate) fn native(cause: NativeObservationError) -> Self {
        Self { cause, identity_unrestored: false }
    }
    /// Recognize an expected original admission veto without replacing native failure causes.
    pub(crate) fn admission_stop(&self) -> Option<crate::task::Stop> {
        use sugarsubstitute_contracts::execution::{AdmissionError, NativeResourceError};
        let cause = std::error::Error::source(&self.cause)?.downcast_ref::<AdmissionError>()?;
        match cause {
            AdmissionError::DeadlineExpired { .. } => Some(crate::task::Stop::TimedOut),
            AdmissionError::ScopeClosed { .. }
            | AdmissionError::ShuttingDown
            | AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired) => {
                Some(crate::task::Stop::Cancelled)
            }
            _ => None,
        }
    }
    /// Return whether Windows failed to restore the native owner's original identity.
    /// The composing process must shut down; this failure is never a retryable peer rejection.
    pub const fn requires_process_shutdown(&self) -> bool {
        self.identity_unrestored
    }
    /// Preserve authoritative-owner panic classification independently of cause text.
    pub fn contains_owner_panic(&self) -> bool {
        self.cause.contains_owner_panic()
    }
    /// Borrow the original redacted native error and its owned source chain.
    pub const fn native_cause(&self) -> &NativeObservationError {
        &self.cause
    }
}
impl fmt::Debug for LifecyclePipeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LifecyclePipeFailure")
            .field("stage", &self.cause.stage())
            .field("identity_unrestored", &self.identity_unrestored)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for LifecyclePipeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Lifecycle pipe native operation failed")
    }
}
impl std::error::Error for LifecyclePipeFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

#[cfg(windows)]
impl From<crate::native_pipe::NativePipeError> for LifecyclePipeFailure {
    fn from(error: crate::native_pipe::NativePipeError) -> Self {
        Self { cause: error.cause, identity_unrestored: error.identity_unrestored }
    }
}
