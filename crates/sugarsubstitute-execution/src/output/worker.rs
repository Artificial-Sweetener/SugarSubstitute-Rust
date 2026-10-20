// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance two bounded native reads with control checkpoints before each effect.
use super::*;
use sugarsubstitute_contracts::execution::{JobOutcome, Moment, PanicCause};
pub(crate) struct Active {
    work: OutputWork,
    context: JobContext,
    started: Moment,
    lease: Option<Box<dyn OutputLease>>,
    signals: [Arc<Signal>; 2],
    buffers: [Option<Buffer>; 2],
    pending: [bool; 2],
    initialized: bool,
    prepared: bool,
    child_released: bool,
    next_stream: usize,
    outcome: Option<JobOutcome<WindowsOutputReport, NativeObservationError>>,
    completed: bool,
    continuation: bool,
}
impl Active {
    pub(crate) fn new(
        mut work: OutputWork,
        context: JobContext,
        core: &Arc<Core>,
        now: Moment,
    ) -> Self {
        let lease = work.lease.take();
        Self {
            work,
            context,
            started: now,
            lease,
            signals: std::array::from_fn(|_| Signal::new(core)),
            buffers: [None, None],
            pending: [false; 2],
            initialized: false,
            prepared: false,
            child_released: false,
            next_stream: 0,
            outcome: None,
            completed: false,
            continuation: false,
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        crate::native::record_owner_panic(&mut self.outcome, cause);
    }
    fn stop(&mut self, now: Moment) -> bool {
        if self.outcome.is_none() {
            if let Some(reason) = crate::task::stop_reason_at(&self.context, now) {
                self.outcome = Some(crate::task::stopped_outcome(reason));
            } else if lock(&self.work.shared.state).stopped {
                self.outcome = Some(JobOutcome::Cancelled);
            }
        }
        self.outcome.is_some()
    }
    fn fail(&mut self, error: NativeObservationError) {
        crate::native::retain_retirement(&mut self.outcome, error);
    }
    fn initialize(&mut self) -> Result<(), NativeObservationError> {
        let memory = self.context.memory().clone();
        let metadata = memory.claim(128 * 1024).map_err(|e| error("output_metadata_claim", e))?;
        {
            let mut s = lock(&self.work.shared.state);
            s.metadata = Some(metadata);
            s.memory = Some(memory.clone());
            s.queue = VecDeque::with_capacity(MAX_WINDOWS_OUTPUT_CHUNKS);
            if s.queue.capacity() * std::mem::size_of::<Chunk>() > 64 * 1024 {
                return Err(invariant("output_queue_capacity"));
            }
        }
        for buffer in &mut self.buffers {
            *buffer = Some(Buffer::new(&memory, &[0; MAX_BATCH_BYTES])?);
        }
        self.initialized = true;
        let handles = self
            .lease
            .as_mut()
            .ok_or_else(|| invariant("output_missing_lease"))?
            .prepare(self.work.shared.identity, self.signals.clone())?;
        if handles.contains(&0) {
            return Err(invariant("output_stdio_handles"));
        }
        {
            let mut s = lock(&self.work.shared.state);
            s.handles = handles;
            s.prepared = true;
        }
        self.prepared = true;
        Ok(())
    }
    fn publish(&self, core: &Core) {
        if core.mailbox.signal_capacity() {
            core.mailbox.deliver_wake();
        }
    }
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        self.continuation = false;
        let fresh = if clock_available { core.clock.now() } else { now };
        self.stop(fresh);
        if !self.initialized && self.outcome.is_none() {
            if let Err(error) = self.initialize() {
                self.fail(error);
            } else {
                self.publish(core);
            }
        }
        if self.prepared && self.outcome.is_none() && !self.child_released {
            let s = lock(&self.work.shared.state);
            let release = s.stdio_taken && !s.pin;
            drop(s);
            if release && let Some(lease) = &mut self.lease {
                match lease.release_child_ends() {
                    Ok(()) => self.child_released = true,
                    Err(e) => self.fail(e),
                }
            }
        }
        if self.outcome.is_none()
            && lock(&self.work.shared.state).input_close_requested
            && let Some(lease) = &mut self.lease
            && let Err(error) = lease.close_input()
        {
            self.fail(error);
        }
        if self.outcome.is_none()
            && let Err(error) = self.read_turn(core, now, clock_available)
        {
            self.fail(error);
        }
        if self.outcome.is_none()
            && lock(&self.work.shared.state).eof_sequence.iter().all(Option::is_some)
        {
            self.outcome = Some(JobOutcome::Success(
                lock(&self.work.shared.state).report(self.work.shared.identity),
            ));
        }
        if self.outcome.is_some() {
            self.retire(core, now, clock_available);
        } else if self.continuation
            && let Some(thread) = core.native_thread.get()
        {
            thread.unpark();
        }
    }
    fn read_turn(
        &mut self,
        core: &Core,
        now: Moment,
        clock_available: bool,
    ) -> Result<(), NativeObservationError> {
        for offset in 0..2 {
            let i = (self.next_stream + offset) % 2;
            let stream = if i == 0 { OutputStream::Stdout } else { OutputStream::Stderr };
            if lock(&self.work.shared.state).eof_sequence[i].is_some() {
                continue;
            }
            if self.stop(if clock_available { core.clock.now() } else { now }) {
                break;
            }
            let lease = self.lease.as_mut().ok_or_else(|| invariant("output_missing_lease"))?;
            let issuing = !self.pending[i];
            if issuing && lock(&self.work.shared.state).pending_read_issues[i] == u64::MAX {
                return Err(invariant("output_pending_issue_exhausted"));
            }
            let progress = if self.pending[i] {
                if !self.signals[i].completed() {
                    continue;
                }
                let buffer =
                    self.buffers[i].as_mut().ok_or_else(|| invariant("output_read_buffer"))?;
                lease.poll(stream, &mut buffer.bytes)?
            } else {
                let buffer =
                    self.buffers[i].as_mut().ok_or_else(|| invariant("output_read_buffer"))?;
                lease.read(stream, &mut buffer.bytes)?
            };
            if !matches!(progress, Progress::Pending | Progress::IssuedPending) {
                lock(&self.work.shared.state).pending_reads[i] = false;
            }
            match progress {
                Progress::Pending => self.pending[i] = true,
                Progress::IssuedPending => {
                    self.pending[i] = true;
                    let mut s = lock(&self.work.shared.state);
                    s.pending_reads[i] = true;
                    if !issuing {
                        return Err(invariant("output_pending_issue_from_poll"));
                    }
                    s.pending_read_issues[i] = s.pending_read_issues[i]
                        .checked_add(1)
                        .ok_or_else(|| invariant("output_pending_issue_exhausted"))?;
                    drop(s);
                    self.publish(core);
                }
                Progress::Empty => {
                    self.pending[i] = false;
                    lease.quiesce(stream)?;
                    let _ = lease.take_completed_read(stream);
                    self.continuation = true;
                }
                Progress::Bytes(bytes) => {
                    self.pending[i] = false;
                    lease.quiesce(stream)?;
                    let _ = lease.take_completed_read(stream);
                    let buffer =
                        self.buffers[i].as_ref().ok_or_else(|| invariant("output_read_buffer"))?;
                    if bytes == 0 || bytes as usize > buffer.bytes.len() {
                        return Err(invariant("output_read_count"));
                    }
                    lock(&self.work.shared.state).push(stream, &buffer.bytes[..bytes as usize])?;
                    self.continuation = true;
                    self.publish(core);
                }
                Progress::Eof => {
                    self.pending[i] = false;
                    lease.quiesce(stream)?;
                    let _ = lease.take_completed_read(stream);
                    lock(&self.work.shared.state).eof(stream)?;
                    self.publish(core);
                }
            }
        }
        self.next_stream = 1 - self.next_stream;
        Ok(())
    }
    fn retire(&mut self, core: &Core, now: Moment, clock_available: bool) {
        let pinned = {
            let mut state = lock(&core.state);
            if let Some(record) = state.jobs.get_mut(&self.context.job_id()) {
                record
                    .native_deadlines
                    .get_or_insert_with(crate::native_deadlines::Deadlines::default)
                    .stop();
            }
            let mut s = lock(&self.work.shared.state);
            s.stopped = true;
            s.pin
        };
        let result = self.lease.as_mut().map_or(Ok(true), |lease| lease.retire(pinned));
        match result {
            Ok(false) => return,
            Err(error) => {
                self.fail(error.clone());
                core.mailbox.record_failure(
                    sugarsubstitute_contracts::execution::ExecutionError::new(
                        self.context.spec().operation,
                        Box::new(error),
                    ),
                );
                core.mailbox.deliver_wake();
                return;
            }
            Ok(true) => {}
        }
        for stream in [OutputStream::Stdout, OutputStream::Stderr] {
            if let Some(progress) =
                self.lease.as_mut().and_then(|lease| lease.take_completed_read(stream))
            {
                let result = match progress {
                    Progress::Bytes(bytes) => self.buffers[stream.index()]
                        .as_ref()
                        .ok_or_else(|| invariant("output_retired_buffer"))
                        .and_then(|buffer| {
                            if bytes as usize > buffer.bytes.len() || bytes == 0 {
                                Err(invariant("output_retired_read_count"))
                            } else {
                                lock(&self.work.shared.state)
                                    .push(stream, &buffer.bytes[..bytes as usize])
                            }
                        }),
                    Progress::Eof => lock(&self.work.shared.state).eof(stream),
                    Progress::Empty => Ok(()),
                    Progress::Pending | Progress::IssuedPending => {
                        Err(invariant("output_unsettled_retired_read"))
                    }
                };
                if let Err(error) = result {
                    self.fail(error);
                } else {
                    self.publish(core);
                }
            }
        }
        drop(self.lease.take());
        self.buffers = [None, None];
        {
            let mut s = lock(&self.work.shared.state);
            s.handles = [0; 3];
            s.retired = true;
            s.pending_reads = [false; 2];
        }
        let ended = if clock_available { core.clock.now() } else { now };
        if let Some(mut outcome) = self.outcome.take() {
            if matches!(outcome, JobOutcome::Success(_)) {
                outcome = JobOutcome::Success(
                    lock(&self.work.shared.state).report(self.work.shared.identity),
                );
            }
            self.work.job.finish_at(&self.context, Some(self.started), ended, outcome);
        }
        self.completed = true;
    }
}
