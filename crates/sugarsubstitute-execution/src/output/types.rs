// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe bounded native stream facts without equating EOF with process settlement.
use sugarsubstitute_contracts::lifecycle::ResourceKey;
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
    ForeignResource,
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
    pub resource: ResourceKey,
    /// Native pipes and all three child handles were prepared.
    pub prepared: bool,
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
