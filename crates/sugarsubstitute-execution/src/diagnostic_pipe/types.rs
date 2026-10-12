// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep kernel-verified helper facts separate from process and storage authority.
use crate::lifecycle_pipe::LifecyclePipeOperationId;
use std::fmt;
use sugarsubstitute_contracts::{incident::DiagnosticKey, lifecycle::ProcessKey};
/// Kernel-verified connection facts; construction is private to the transport owner.
/// Facts grant no termination, profile, resource or UI-loop authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedDiagnosticPeer {
    pub(crate) key: DiagnosticKey,
    pub(crate) process: Option<ProcessKey>,
    pub(crate) pid: u32,
    pub(crate) birth: u64,
    pub(crate) session: u32,
}
impl VerifiedDiagnosticPeer {
    /// Return the exact admitted helper session whose bootstrap and native peer were verified.
    pub const fn key(self) -> DiagnosticKey {
        self.key
    }
    /// Return the enrolled owned helper registry key on the server side.
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
pub enum DiagnosticPipeObservation {
    /// Server pipe and immutable bootstrap section actually exist.
    Prepared,
    /// Actual pipe connection exists; application authentication is still pending.
    Connected,
    /// Kernel identity and per-session capability were verified on this connection.
    Authenticated(VerifiedDiagnosticPeer),
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
        frame: crate::lifecycle_pipe::ReceivedLifecycleFrame,
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
        buffer: crate::lifecycle_pipe::LifecyclePipeWriteBuffer,
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
impl fmt::Debug for DiagnosticPipeObservation {
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
pub struct DiagnosticPipeReport {
    /// The admitted helper session, if a child bootstrap was successfully validated.
    pub key: Option<DiagnosticKey>,
    /// Number of completely observed messages, without claiming semantic effects.
    pub received_messages: u64,
    /// Number of completely observed writes, without claiming peer processing.
    pub completed_writes: u64,
}
