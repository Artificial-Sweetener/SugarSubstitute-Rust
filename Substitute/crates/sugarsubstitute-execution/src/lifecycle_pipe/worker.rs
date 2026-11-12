// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance admitted pipe commands and retain storage until actual native retirement.
use super::*;
use sugarsubstitute_contracts::execution::{ExecutionError, JobOutcome, PanicCause};
#[path = "protocol.rs"]
mod protocol;
#[path = "write_revocation.rs"]
mod write_revocation;
pub(crate) struct ActiveState {
    work: PipeWork,
    context: JobContext,
    started: Moment,
    lease: Option<Box<dyn PipeLease>>,
    signals: [Arc<Signal>; 3],
    initialized: bool,
    prepared: bool,
    enrolled: bool,
    connecting: bool,
    connected: bool,
    read: Option<(ReadCommand, buffers::Buffer)>,
    write: Option<Write>,
    outcome: Option<JobOutcome<(), LifecyclePipeFailure>>,
    completed: bool,
    native_retired: bool,
    retirement_published: bool,
}
impl ActiveState {
    /// Count the actual boxed worker and outside private shared/control metadata.
    pub(crate) const fn storage_bytes() -> usize {
        size_of::<Self>()
            + shared::SharedRef::control_storage_bytes()
            + metadata::Metadata::control_storage_bytes()
            + 8 * size_of::<Observation>()
            + size_of::<crate::task::Job<(), LifecyclePipeFailure>>()
            + 2 * size_of::<std::sync::atomic::AtomicUsize>()
            + size_of::<crate::task::JobHandle<(), LifecyclePipeFailure>>()
    }
    pub(crate) fn new(
        mut work: PipeWork,
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
            initialized: false,
            prepared: false,
            enrolled: false,
            connecting: false,
            connected: false,
            read: None,
            write: None,
            outcome: None,
            completed: false,
            native_retired: false,
            retirement_published: false,
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        match &mut self.outcome {
            Some(JobOutcome::Failure(error)) => error.cause.retain_panic(cause),
            Some(JobOutcome::Panicked(_)) => {}
            outcome => *outcome = Some(JobOutcome::Panicked(cause.clone())),
        }
    }
    fn error(&mut self, core: &Core, error: LifecyclePipeFailure) {
        if error.requires_process_shutdown() {
            core.mailbox.suppress_current_thread();
            core.native_identity_unrestored.store(true, std::sync::atomic::Ordering::Release);
            core.shutdown();
            core.mailbox.record_failure(ExecutionError::new(
                self.context.spec().operation,
                Box::new(error.clone()),
            ));
            core.mailbox.deliver_wake();
        }
        if !error.requires_process_shutdown()
            && self.outcome.is_none()
            && let Some(reason) = error.admission_stop()
        {
            self.outcome = Some(crate::task::stopped_outcome(reason));
        } else {
            retain_failure(&mut self.outcome, error);
        }
    }
    fn stop(&mut self, core: &Core, now: Moment) -> bool {
        if self.outcome.is_some() {
            return true;
        }
        if let Some(reason) = crate::task::stop_reason_at(&self.context, now) {
            self.outcome = Some(crate::task::stopped_outcome(reason));
            return true;
        }
        let state = lock(&core.state);
        if let Some(reason) =
            crate::symbol_admission::original_stop(&state, self.context.job_id(), now).or_else(
                || {
                    crate::coordinator_entry::recovery_grant::original_stop(
                        core,
                        &state,
                        self.work.shared.recovery_grant.as_ref(),
                        now,
                    )
                },
            )
        {
            self.outcome = Some(crate::task::stopped_outcome(reason));
            return true;
        }
        let s = lock(&self.work.shared.state);
        if s.stopped {
            self.outcome = Some(JobOutcome::Cancelled);
        } else if state
            .jobs
            .get(&self.context.job_id())
            .and_then(|r| r.native_deadlines.as_ref())
            .is_some_and(|slots| slots.expired(now))
        {
            self.outcome = Some(JobOutcome::TimedOut);
        }
        self.outcome.is_some()
    }
    fn publish(&self, core: &Core) {
        if core.mailbox.signal_capacity() {
            core.mailbox.deliver_wake();
        }
    }
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        // Adopt transferred handles even when cancellation won before preparation.
        let peer = { lock(&self.work.shared.state).peer.take() };
        if let Some(peer) = peer
            && let Some(lease) = &mut self.lease
        {
            match lease.adopt(peer) {
                Ok(()) => self.enrolled = true,
                Err((error, peer)) => {
                    lock(&self.work.shared.state).peer = Some(peer);
                    self.error(core, *error);
                }
            }
        }
        let observed = if clock_available { core.clock.now() } else { now };
        self.stop(core, observed);
        if !self.initialized && self.outcome.is_none() {
            self.initialized = true;
            if let Err(error) = allocate(&self.context, &self.work.shared) {
                self.error(core, error);
            }
        }
        let server = lock(&self.work.shared.state).binding.is_some();
        if !self.prepared && self.outcome.is_none() && (server || self.enrolled) {
            if self.stop(core, core.clock.now()) {
                self.retirement(core, now, clock_available);
                return;
            }
            let result = self.lease.as_mut().ok_or_else(|| failure("pipe_missing_lease")).and_then(
                |lease| {
                    lease.prepare(
                        self.signals.clone(),
                        &self.context,
                        self.work.shared.original_deadline,
                    )
                },
            );
            match result {
                Ok(inherited) => {
                    self.prepared = true;
                    let binding = self.lease.as_ref().and_then(|lease| lease.binding());
                    let deadline = self.lease.as_ref().and_then(|lease| lease.deadline());
                    let capability = self.lease.as_ref().and_then(|lease| lease.capability());
                    #[cfg(windows)]
                    let capture_inherited =
                        self.lease.as_ref().and_then(|lease| lease.inherited_capture_processes());
                    let mut s = lock(&self.work.shared.state);
                    if binding.is_none() || (s.binding.is_some() && s.binding != binding) {
                        drop(s);
                        self.error(core, failure("pipe_prepared_binding"));
                        self.retirement(core, now, clock_available);
                        return;
                    }
                    s.inherited = inherited;
                    #[cfg(windows)]
                    {
                        s.capture_inherited = capture_inherited;
                    }
                    s.binding = binding;
                    s.prepared = true;
                    s.deadline = deadline;
                    s.capability = capability;
                    s.observations.push_back(Observation::Prepared);
                    drop(s);
                    self.publish(core);
                }
                Err(error) => self.error(core, error),
            }
        }
        if self.outcome.is_none()
            && self.prepared
            && self.enrolled
            && let Err(error) = self.protocol(core)
        {
            self.error(core, error);
        }
        if self.outcome.is_some() {
            self.retirement(
                core,
                now,
                clock_available
                    && !core.native_identity_unrestored.load(std::sync::atomic::Ordering::Acquire),
            );
        }
    }
    fn retirement(&mut self, core: &Core, now: Moment, clock_available: bool) {
        #[cfg(windows)]
        {
            let captured = { lock(&self.work.shared.state).captured_startup.take() };
            drop(captured);
        }
        let (pinned, peer, abandoned, authentication, mut capability) = {
            let mut state = lock(&core.state);
            let mut s = lock(&self.work.shared.state);
            s.stopped = true;
            if let Some(slots) =
                state.jobs.get_mut(&self.context.job_id()).and_then(|r| r.native_deadlines.as_mut())
            {
                slots.stop();
            }
            let mut abandoned = std::collections::VecDeque::with_capacity(8);
            // Preserve passive actual facts through terminal; only buffer guards
            // require release before native/session settlement can complete.
            for _ in 0..s.observations.len() {
                if let Some(fact) = s.observations.pop_front() {
                    if matches!(
                        fact,
                        Observation::Received { .. } | Observation::WriteRevoked { .. }
                    ) {
                        abandoned.push_back(fact);
                    } else {
                        s.observations.push_back(fact);
                    }
                }
            }
            (
                s.pin || s.input_pin,
                s.peer.take(),
                abandoned,
                s.authenticate.take(),
                s.capability.take(),
            )
        };
        // Publication releases input_pin under this same lock. Take the peer
        // with the pin snapshot so retirement cannot skip a late transfer.
        if let Some(peer) = peer {
            if let Some(lease) = &mut self.lease {
                match lease.adopt(peer) {
                    Ok(()) => self.enrolled = true,
                    Err((error, peer)) => {
                        lock(&self.work.shared.state).peer = Some(peer);
                        self.error(core, *error);
                        return;
                    }
                }
            } else {
                lock(&self.work.shared.state).peer = Some(peer);
                self.error(core, failure("pipe_missing_lease"));
                return;
            }
        }
        // Guard destruction may take the session lock and release a real memory claim.
        drop(abandoned);
        drop(authentication);
        if let Some(capability) = &mut capability {
            capability.fill(0);
        }
        let result = if self.native_retired {
            Ok(true)
        } else {
            self.lease.as_mut().map_or(Ok(true), |lease| lease.retire(pinned))
        };
        match result {
            Ok(false) => return,
            Err(error) => {
                retain_failure(&mut self.outcome, error.clone());
                core.mailbox.record_failure(ExecutionError::new(
                    self.context.spec().operation,
                    Box::new(error),
                ));
                core.mailbox.deliver_wake();
                return;
            }
            Ok(true) => self.native_retired = true,
        }
        if let Some(bytes) = self.lease.as_mut().and_then(|lease| lease.take_completed_write())
            && let Some(write) = &self.write
        {
            if bytes as usize == write.length {
                let mut state = lock(&self.work.shared.state);
                state.written += 1;
                state.write_slots = 0;
                state
                    .observations
                    .push_back(Observation::Written { operation: write.command.operation, bytes });
                drop(state);
                self.publish(core);
            } else {
                retain_failure(&mut self.outcome, failure("pipe_partial_write"));
            }
        }
        if lock(&self.work.shared.state).loans != 0 || pinned {
            return;
        }
        self.read = None;
        self.write = None;
        let (queued_read, queued_write, available_read, available_write) = {
            let mut s = lock(&self.work.shared.state);
            (s.read.take(), s.write.take(), s.read_buffer.take(), s.write_buffer.take())
        };
        drop((queued_read, queued_write, available_read, available_write));
        drop(self.lease.take());
        if !self.retirement_published {
            let mut state = lock(&self.work.shared.state);
            state.checked_retirement = Some(super::retirement::CheckedRetirement {
                execution: self.context.owner_id(),
                scope: self.context.scope_id(),
                spec: self.context.spec(),
                run: state.binding.and_then(PeerBinding::desktop),
            });
            self.retirement_published = true;
        }
        let ended = if clock_available { core.clock.now() } else { now };
        self.completed = true;
        if let Some(outcome) = self.outcome.take() {
            let facts = {
                let state = lock(&self.work.shared.state);
                owner::RetiredFacts {
                    binding: state.binding,
                    received: state.received,
                    written: state.written,
                }
            };
            self.work.owner.finish(&self.context, self.started, ended, outcome, facts);
        }
    }
}

/// Keep every buffer, lease and original cause together when settlement is unproved.
pub(crate) struct Active {
    owner: Option<Box<ActiveState>>,
    _metadata: metadata::Metadata,
}
impl Active {
    pub(crate) fn new(work: PipeWork, context: JobContext, core: &Arc<Core>, now: Moment) -> Self {
        let metadata = work.shared.metadata.clone();
        Self {
            owner: Some(Box::new(ActiveState::new(work, context, core, now))),
            _metadata: metadata,
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.owner.as_ref().is_some_and(|owner| owner.completed())
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        if let Some(owner) = &mut self.owner {
            owner.fail_owner(cause);
        }
    }
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        if let Some(owner) = &mut self.owner {
            owner.progress(core, now, clock_available);
        }
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take()
            && !owner.completed
        {
            let retained = Box::leak(owner);
            std::mem::forget(self._metadata.clone());
            tracing::error!(
                operation = "native_pipe_uncertain_drop",
                job_id = retained.context.job_id().get(),
                "Native pipe buffers, output custody and original causes retained before diagnostics"
            );
        }
    }
}
