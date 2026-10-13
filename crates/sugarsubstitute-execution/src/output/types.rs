// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe bounded native stream facts without equating EOF with process settlement.
use sugarsubstitute_contracts::{
    lifecycle::{ResourceKey, RunKey},
    transcript::{OutputGap, OutputStream, StreamSequence},
};
/// Exact owned launch association; matching facts grant no process authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputIdentity {
    /// Output from one managed backend resource generation.
    Managed(ResourceKey),
    /// Startup and runtime diagnostics from one desktop run generation.
    Desktop(RunKey),
}
/// One bounded desktop stream fact, borrowed only through its charged delivery guard.
/// Desktop diagnostics remain distinct from managed backend Transcript input.
#[derive(Debug)]
pub struct DesktopOutputBatch {
    pub(super) run: RunKey,
    pub(super) stream: OutputStream,
    pub(super) sequence: StreamSequence,
    pub(super) bytes: Box<[u8]>,
    pub(super) gap: Option<OutputGap>,
    pub(super) end: bool,
}
impl DesktopOutputBatch {
    /// Return the exact desktop generation, without a process capability.
    pub const fn run(&self) -> RunKey {
        self.run
    }
    /// Identify stdout or stderr independently.
    pub const fn stream(&self) -> OutputStream {
        self.stream
    }
    /// Return the unchanged checked per-stream sequence.
    pub const fn sequence(&self) -> StreamSequence {
        self.sequence
    }
    /// Borrow raw source bytes; no decoding or logging occurs here.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Return preceding bounded-queue loss for this stream.
    pub const fn gap(&self) -> Option<OutputGap> {
        self.gap
    }
    /// Report actual producer EOF, independently of process settlement.
    pub const fn end(&self) -> bool {
        self.end
    }
}
/// Combined raw queued payload bound, independent of visibility.
pub const MAX_WINDOWS_OUTPUT_QUEUED_BYTES: usize = 1024 * 1024;
/// Fixed queue index bound, including tiny read completions.
pub const MAX_WINDOWS_OUTPUT_CHUNKS: usize = 32;
/// Admit queue, two reads, native/index metadata and one copied delivered guard.
pub const MIN_WINDOWS_OUTPUT_BYTES: u64 = (1024 * 1024 + 256 * 1024) as u64;
/// Reject a typed control operation without exposing foreign output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsOutputError {
    /// Native construction has not published its checked handles.
    NotReady,
    /// Cancellation has invalidated launch admission.
    Closed,
    /// The sole launch artifact was already taken.
    AlreadyTransferred,
    /// Another admitted batch still owns the delivery slot.
    LoanBusy,
    /// The caller supplied another resource or acquisition generation.
    ForeignIdentity,
    /// Original charged storage could not cover a bounded copy.
    MemoryBusy,
    /// Bounded output sequence/counter cannot advance.
    Exhausted,
}
impl std::fmt::Display for WindowsOutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Owned output operation rejected")
    }
}
impl std::error::Error for WindowsOutputError {}
/// Actual per-stream byte and loss accounting; bytes are raw source bytes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WindowsOutputCounters {
    /// Raw bytes actually read from this pipe.
    pub read_bytes: u64,
    /// Actual bytes discarded by bounded queue or held-loan pressure.
    pub dropped_bytes: u64,
    /// Complete sequence positions discarded, excluding synthetic gap/EOF facts.
    pub dropped_batches: u64,
}
/// Snapshot captured output facts; neither EOF nor retirement proves family settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsOutputReport {
    /// Exact coordinator/resource generation supplied by the trusted owner.
    pub identity: OutputIdentity,
    /// Native pipes and all three child handles were prepared.
    pub prepared: bool,
    /// Retained evidence of actual ERROR_IO_PENDING reads, not a fresh kernel query.
    /// Normal observed results clear it; stopping may retain it until the whole
    /// native owner completes checked retirement, including a held launch pin.
    pub pending_reads: [bool; 2],
    /// Checked counts of actual ERROR_IO_PENDING issue results, per stream.
    /// Admission, polling and ERROR_IO_INCOMPLETE do not increment these.
    pub pending_read_issues: [u64; 2],
    /// Independently observed stdout/stderr EOF, never inferred from close/cancel.
    pub eof: [bool; 2],
    /// Fixed stdout/stderr raw counters.
    pub counters: [WindowsOutputCounters; 2],
    /// Currently retained queued payload, excluding a delivered guard.
    pub queued_bytes: u64,
    /// Fixed-index queue occupancy.
    pub queued_chunks: u32,
    /// Tail losses still awaiting a retained batch, gap-only fact or actual EOF.
    pub pending_loss: [u64; 2],
    /// Native checked retirement has finished; unrelated processes may still live.
    pub retired: bool,
}
