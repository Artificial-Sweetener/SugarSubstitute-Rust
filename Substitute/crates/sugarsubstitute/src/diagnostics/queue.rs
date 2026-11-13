// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain only fixed-count, fixed-byte redacted frames under actual execution memory custody.
use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};
#[cfg(any(windows, test))]
use sugarsubstitute_contracts::diagnostics::DIAGNOSTIC_BATCH_EVENTS;
use sugarsubstitute_contracts::{
    diagnostics::{DIAGNOSTIC_EVENT_BYTES, DIAGNOSTIC_QUEUE_EVENTS},
    execution::{ExecutionId, MemoryClaim},
};
pub(super) const EVENT_BYTES: usize = DIAGNOSTIC_EVENT_BYTES;
pub(super) const QUEUE_EVENTS: usize = DIAGNOSTIC_QUEUE_EVENTS;
/// Share bounded frame custody and one monotonic shutdown cutoff with every admitted batch.
pub(super) struct Queue(pub(super) Mutex<State>, Arc<AtomicU64>);
/// Serialize producer admission, loss accounting and original execution memory custody.
pub(super) struct State {
    frames: VecDeque<String>,
    omitted: u64,
    pub(super) stopping: bool,
    pub(super) execution: Option<ExecutionId>,
    pub(super) memory: Option<Arc<MemoryClaim>>,
}
/// Keep the original allocation with drained frames through rejected admission and actual worker return.
#[cfg(any(windows, test))]
pub(super) struct Batch {
    pub(super) frames: Vec<String>,
    pub(super) omitted: u64,
    pub(super) finished: bool,
    pub(super) cutoff: Arc<AtomicU64>,
    _memory: Option<Arc<MemoryClaim>>,
}
impl State {
    #[cfg(windows)]
    /// Drop queued allocations before retiring their original memory reservation.
    pub(super) fn clear(&mut self) {
        self.frames.clear();
        self.frames.shrink_to_fit();
        drop(self.memory.take());
    }
}
impl Queue {
    #[cfg(any(windows, test))]
    /// Create an unadmitted route; the execution pump installs its actual memory reservation.
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self(
            Mutex::new(State {
                frames: VecDeque::new(),
                omitted: 0,
                stopping: false,
                execution: None,
                memory: None,
            }),
            Arc::new(AtomicU64::new(u64::MAX)),
        ))
    }
    #[cfg(any(windows, test))]
    /// Reject new events and lower the shared cutoff without extending an earlier stop request.
    pub(super) fn stop(&self, cutoff: sugarsubstitute_contracts::execution::Moment) {
        self.1.fetch_min(cutoff.as_nanos(), Ordering::AcqRel);
        self.state().stopping = true;
    }
    /// Recover protected custody even after poisoning so teardown can release retained allocations.
    pub(super) fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    /// Count rejected frames and suppress this pump's own completion events to prevent recursion.
    pub(super) fn push(&self, frame: String) {
        let mut state = self.state();
        if frame.len() > EVENT_BYTES {
            state.omitted = state.omitted.saturating_add(1);
            return;
        }
        if state.execution.is_some_and(|id| {
            frame.split_ascii_whitespace().any(|field| {
                field.strip_prefix("execution_id=").and_then(|value| value.parse::<u64>().ok())
                    == Some(id.get())
            })
        }) {
            return;
        }
        if state.stopping
            || self.1.load(Ordering::Acquire) != u64::MAX
            || state.memory.is_none()
            || state.frames.len() == QUEUE_EVENTS
        {
            state.omitted = state.omitted.saturating_add(1);
        } else {
            state.frames.push_back(frame);
        }
    }
    #[cfg(any(windows, test))]
    /// Transfer a finite batch while retaining the same original charge and stop cutoff.
    pub(super) fn take(&self) -> Batch {
        let mut state = self.state();
        let count = state.frames.len().min(DIAGNOSTIC_BATCH_EVENTS);
        let frames = state.frames.drain(..count).collect();
        let omitted = std::mem::take(&mut state.omitted);
        Batch {
            frames,
            omitted,
            finished: state.stopping && state.frames.is_empty(),
            cutoff: self.1.clone(),
            _memory: state.memory.clone(),
        }
    }
}
/// Tee an already redacted event to owned stderr and the bounded persistent queue.
pub(super) struct Routed<W: Write> {
    pub(super) sink: W,
    pub(super) queue: Option<Arc<Queue>>,
    pub(super) frame: Vec<u8>,
    pub(super) oversized: bool,
    pub(super) failed: bool,
    pub(super) written: usize,
}
impl<W: Write> Routed<W> {
    /// Stop the formatter before nested span contexts can exceed the complete-event byte limit.
    fn admit(&mut self, count: usize) -> io::Result<()> {
        const OMITTED: &[u8] = b" operation=diagnostic_oversized omitted_events=1\n";
        if self.oversized {
            return Err(io::Error::other("Diagnostic event exceeds admission"));
        }
        if count > EVENT_BYTES.saturating_sub(OMITTED.len() + self.written) {
            self.oversized = true;
            if let Err(cause) = self.sink.write_all(OMITTED) {
                self.failed = true;
                return Err(cause);
            }
            self.written += OMITTED.len();
            return Err(io::Error::other("Diagnostic event exceeds admission"));
        }
        Ok(())
    }
    /// Copy only bytes that the real sink accepted, under the complete-event bound.
    fn retain(&mut self, bytes: &[u8]) {
        if self.queue.is_some() {
            if self.frame.len() + bytes.len() <= EVENT_BYTES && !self.oversized {
                self.frame.extend_from_slice(bytes);
            } else {
                self.frame.clear();
                self.oversized = true;
            }
        }
    }
}
impl<W: Write> Write for Routed<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.admit(bytes.len())?;
        let result = self.sink.write(bytes);
        match &result {
            Ok(count) => {
                self.written += *count;
                self.retain(&bytes[..*count]);
            }
            Err(_) => self.failed = true,
        }
        result
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.admit(bytes.len())?;
        let result = self.sink.write_all(bytes);
        if result.is_ok() {
            self.written += bytes.len();
            self.retain(bytes);
        } else {
            self.failed = true;
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        self.sink.flush()
    }
}
impl<W: Write> Drop for Routed<W> {
    fn drop(&mut self) {
        if let Some(queue) = &self.queue {
            let frame = if self.failed {
                "operation=diagnostic_sink_failure incomplete_event=1\n".into()
            } else if self.oversized {
                "operation=diagnostic_oversized omitted_events=1\n".into()
            } else {
                String::from_utf8_lossy(&self.frame).into_owned()
            };
            if !frame.is_empty() {
                queue.push(frame);
            }
        }
    }
}
