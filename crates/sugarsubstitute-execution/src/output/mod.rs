// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drain admitted owned process streams independently of transcript presentation.
mod control;
mod queue;
#[cfg(test)]
mod tests;
mod types;
pub(crate) mod worker;
use crate::{
    admission::{Core, RequestKind},
    native::{NativeObservationError, Signal},
    scope::lock,
    task::{Job, JobHandle, QueuedJob, Work},
};
pub use control::{
    AdmittedOutputBatch, WindowsChildStdio, WindowsOutputCancellation, WindowsOutputControl,
    WindowsOutputSession,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, Weak},
};
use sugarsubstitute_contracts::{
    execution::{AdmissionError, JobContext, JobSpec, MemoryClaim, MemoryLease, OwnedJob, Scope},
    transcript::{MAX_BATCH_BYTES, OutputBatch, OutputGap, OutputStream, StreamSequence},
};
pub use types::*;

#[derive(Clone, Copy)]
pub(crate) enum Progress {
    Pending,
    IssuedPending,
    Bytes(u32),
    Empty,
    Eof,
}
pub(crate) trait OutputLease: Send {
    fn prepare(
        &mut self,
        identity: OutputIdentity,
        signals: [Arc<Signal>; 2],
    ) -> Result<[usize; 3], NativeObservationError>;
    fn release_child_ends(&mut self) -> Result<(), NativeObservationError>;
    fn read(
        &mut self,
        stream: OutputStream,
        bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError>;
    fn poll(
        &mut self,
        stream: OutputStream,
        bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError>;
    fn quiesce(&mut self, stream: OutputStream) -> Result<(), NativeObservationError>;
    fn take_completed_read(&mut self, stream: OutputStream) -> Option<Progress>;
    fn retire(&mut self, pinned: bool) -> Result<bool, NativeObservationError>;
}
pub(crate) struct Buffer {
    bytes: Vec<u8>,
    _claim: MemoryClaim,
}
impl Buffer {
    fn new(memory: &MemoryLease, source: &[u8]) -> Result<Self, NativeObservationError> {
        let claim =
            memory.claim(MAX_BATCH_BYTES as u64).map_err(|e| error("output_chunk_claim", e))?;
        let mut bytes = Vec::with_capacity(source.len());
        if bytes.capacity() > MAX_BATCH_BYTES {
            return Err(invariant("output_chunk_capacity"));
        }
        bytes.extend_from_slice(source);
        Ok(Self { bytes, _claim: claim })
    }
}
enum Batch {
    Managed(OutputBatch),
    Desktop(DesktopOutputBatch),
}
impl Batch {
    fn new(
        identity: OutputIdentity,
        stream: OutputStream,
        sequence: StreamSequence,
        bytes: &[u8],
        gap: Option<OutputGap>,
        end: bool,
    ) -> Result<Self, sugarsubstitute_contracts::transcript::TranscriptError> {
        match identity {
            OutputIdentity::Managed(resource) => {
                OutputBatch::new(resource, stream, sequence, bytes, gap, end).map(Self::Managed)
            }
            OutputIdentity::Desktop(run) => {
                if bytes.len() > MAX_BATCH_BYTES {
                    return Err(sugarsubstitute_contracts::transcript::TranscriptError::Bound);
                }
                Ok(Self::Desktop(DesktopOutputBatch {
                    run,
                    stream,
                    sequence,
                    bytes: bytes.into(),
                    gap,
                    end,
                }))
            }
        }
    }
}
pub(crate) struct Chunk {
    stream: OutputStream,
    sequence: u64,
    buffer: Buffer,
    gap: Loss,
}
#[derive(Clone, Copy, Default)]
pub(crate) struct Loss {
    bytes: u64,
    batches: u64,
}
pub(crate) struct State {
    prepared: bool,
    stopped: bool,
    retired: bool,
    pending_reads: [bool; 2],
    pending_read_issues: [u64; 2],
    stdio_taken: bool,
    pin: bool,
    handles: [usize; 3],
    loan: bool,
    queue: VecDeque<Chunk>,
    queued_bytes: usize,
    sequences: [u64; 2],
    tail: [Loss; 2],
    eof_sequence: [Option<u64>; 2],
    eof_delivered: [bool; 2],
    gap_delivered: [bool; 2],
    counters: [WindowsOutputCounters; 2],
    memory: Option<MemoryLease>,
    metadata: Option<MemoryClaim>,
}
impl State {
    fn new() -> Self {
        Self {
            prepared: false,
            stopped: false,
            retired: false,
            pending_reads: [false; 2],
            pending_read_issues: [0; 2],
            stdio_taken: false,
            pin: false,
            handles: [0; 3],
            loan: false,
            queue: VecDeque::new(),
            queued_bytes: 0,
            sequences: [0; 2],
            tail: [Loss::default(); 2],
            eof_sequence: [None; 2],
            eof_delivered: [false; 2],
            gap_delivered: [false; 2],
            counters: [WindowsOutputCounters::default(); 2],
            memory: None,
            metadata: None,
        }
    }
    fn report(&self, identity: OutputIdentity) -> WindowsOutputReport {
        WindowsOutputReport {
            identity,
            prepared: self.prepared,
            pending_reads: self.pending_reads,
            pending_read_issues: self.pending_read_issues,
            eof: self.eof_sequence.map(|s| s.is_some()),
            counters: self.counters,
            queued_bytes: self.queued_bytes as u64,
            queued_chunks: self.queue.len() as u32,
            pending_loss: self.tail.map(|l| l.bytes),
            retired: self.retired,
        }
    }
}
pub(crate) struct Shared {
    state: Mutex<State>,
    identity: OutputIdentity,
    core: Weak<Core>,
}
impl Shared {
    fn changed(&self) {
        if let Some(core) = self.core.upgrade() {
            core.changed();
        }
    }
}
pub(crate) struct OutputWork {
    shared: Arc<Shared>,
    job: Arc<Job<WindowsOutputReport, NativeObservationError>>,
    lease: Option<Box<dyn OutputLease>>,
}
pub(crate) fn reserve(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    identity: OutputIdentity,
    lease: Box<dyn OutputLease>,
) -> Result<WindowsOutputSession, AdmissionError> {
    if spec.reservation_bytes < MIN_WINDOWS_OUTPUT_BYTES {
        return Err(AdmissionError::NativeReservationTooSmall {
            requested_bytes: spec.reservation_bytes,
            minimum_bytes: MIN_WINDOWS_OUTPUT_BYTES,
        });
    }
    core.admit(scope, spec, RequestKind::Native, |context, cancel, timing| {
        let admitted = timing.admitted;
        let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(core));
        let shared = Arc::new(Shared {
            state: Mutex::new(State::new()),
            identity,
            core: Arc::downgrade(core),
        });
        let queued = QueuedJob {
            context,
            job: job.clone(),
            work: Work::Native(crate::native::NativeWork::Output(OutputWork {
                shared: shared.clone(),
                job: job.clone(),
                lease: Some(lease),
            })),
        };
        let session = WindowsOutputSession {
            control: WindowsOutputControl { shared, job: job.clone() },
            completion: OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job))),
        };
        (queued, session)
    })
}
pub(crate) fn invariant(stage: &'static str) -> NativeObservationError {
    error(stage, std::io::Error::other("Owned output boundary invariant rejected"))
}
pub(crate) fn error(
    stage: &'static str,
    cause: impl std::error::Error + Send + Sync + 'static,
) -> NativeObservationError {
    NativeObservationError::new(stage, cause)
}
