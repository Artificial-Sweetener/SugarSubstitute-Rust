// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance fixed entry connections on the existing native worker and retain actual custody.
use super::*;
use crate::native_deadlines::Role;
use sugarsubstitute_contracts::{
    coordinator_entry::*,
    execution::{ExecutionError, JobOutcome, PanicCause},
    lifecycle::ShutdownBudget,
};
mod advance;
mod allocation;
mod cleanup;
mod commands;
mod handshake;
mod retirement;
mod source_staging;
#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Available,
    Connecting,
    Hello,
    Challenge,
    Launch,
    Outcome,
    AwaitRoot,
    Retiring,
}
#[derive(Clone, Copy)]
struct Io {
    role: Role,
    length: usize,
    next: Phase,
    deadline: Option<Moment>,
    operation: Option<EntryOperationId>,
}
struct ConnectionIo {
    phase: Phase,
    pending: Option<Io>,
    read: Option<buffers::Buffer>,
    write: Option<buffers::Buffer>,
    failure: Option<EntryTransportError>,
    native_retired: bool,
    retry_at: Option<Moment>,
}
impl ConnectionIo {
    fn new() -> Self {
        Self {
            phase: Phase::Available,
            pending: None,
            read: None,
            write: None,
            failure: None,
            native_retired: false,
            retry_at: None,
        }
    }
}
struct ClientStaging {
    header: protocol::Header,
    length: usize,
    hello: [u8; protocol::HEADER_BYTES],
}
pub(crate) struct Active {
    work: EntryWork,
    context: JobContext,
    started: Moment,
    lease: Option<Box<dyn EntryLease>>,
    signals: Vec<[Arc<Signal>; 3]>,
    connections: Vec<ConnectionIo>,
    replay: Option<replay::Replay>,
    schedule: schedule::Schedule,
    client: Option<Box<ClientStaging>>,
    initialized: bool,
    prepared: bool,
    native_retired: bool,
    cleanup: cleanup::Cleanup,
    outcome: Option<JobOutcome<EntryTransportReport, EntryTransportError>>,
    report: EntryTransportReport,
    completed: bool,
    source_pending: Option<source_staging::EntrySourceOutcome>,
    source_owner: Option<crate::entry_preparation::EntryPreparationOwner>,
    source_input_claim: Option<sugarsubstitute_contracts::execution::MemoryClaim>,
}
impl Active {
    pub(crate) fn new(
        mut work: EntryWork,
        context: JobContext,
        core: &Arc<Core>,
        now: Moment,
    ) -> Self {
        let lease = work.lease.take();
        // Signal objects and connection metadata are allocated only after the metadata claim.
        let _ = core;
        Self {
            work,
            context,
            started: now,
            lease,
            signals: Vec::new(),
            connections: Vec::new(),
            replay: None,
            schedule: schedule::Schedule::new(),
            client: None,
            initialized: false,
            prepared: false,
            native_retired: false,
            cleanup: cleanup::Cleanup::Pending,
            outcome: None,
            report: EntryTransportReport {
                retired_connections: 0,
                authenticated_launches: 0,
                completed_writes: 0,
            },
            completed: false,
            source_pending: None,
            source_owner: None,
            source_input_claim: None,
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        self.preserve_pending_source_failure();
        if let Some(JobOutcome::Failure(error)) = &mut self.outcome {
            if let Some(native) = error.native_cause() {
                let mut native = native.clone();
                native.retain_panic(cause);
                error.retain_cleanup(EntryTransportError::native(native));
            } else {
                let mut native = NativeObservationError::new(
                    "entry_owner_panic",
                    std::io::Error::other(cause.as_str().to_owned()),
                );
                native.retain_panic(cause);
                error.retain_cleanup(EntryTransportError::native(native));
            }
        } else if !matches!(self.outcome, Some(JobOutcome::Panicked(_))) {
            self.outcome = Some(JobOutcome::Panicked(cause.clone()));
        }
    }
    fn lease(&mut self) -> Result<&mut (dyn EntryLease + 'static), EntryTransportError> {
        self.lease.as_deref_mut().ok_or(EntryTransportError::InvalidPhase)
    }
    fn error(&mut self, core: &Core, mut error: EntryTransportError) {
        if error.requires_process_shutdown() {
            core.mailbox.suppress_current_thread();
            core.native_identity_unrestored.store(true, std::sync::atomic::Ordering::Release);
            core.shutdown();
        }
        if let Some(JobOutcome::Failure(original)) = &mut self.outcome {
            original.retain_cleanup(error.clone());
        } else {
            if let Some(JobOutcome::Panicked(cause)) = &self.outcome {
                let mut native = NativeObservationError::new(
                    "entry_owner_panic",
                    std::io::Error::other(cause.as_str().to_owned()),
                );
                native.retain_panic(cause);
                error.retain_cleanup(EntryTransportError::native(native));
            }
            self.outcome = Some(JobOutcome::Failure(error.clone()));
        }
        self.diagnostic(core, error);
    }
    fn diagnostic(&self, core: &Core, error: EntryTransportError) {
        core.mailbox
            .record_failure(ExecutionError::new(self.context.spec().operation, Box::new(error)));
        core.mailbox.deliver_wake();
    }
    fn emit(
        &self,
        core: &Core,
        index: Option<usize>,
        fact: EntryTransportObservation,
    ) -> Result<(), EntryTransportError> {
        lock(&self.work.shared.state).emit(index, fact)?;
        if core.mailbox.signal_capacity() {
            core.mailbox.deliver_wake();
        }
        Ok(())
    }
    fn close_connection(&mut self, core: &Core, index: usize, error: Option<EntryTransportError>) {
        if let Some(error) = error {
            if !self.work.shared.server || error.requires_process_shutdown() {
                self.error(core, error);
                return;
            }
            if !matches!(error, EntryTransportError::PeerDisconnected(_))
                || error.contains_owner_panic()
            {
                self.diagnostic(core, error.clone());
            }
            let slot = &mut self.connections[index];
            if let Some(original) = &mut slot.failure {
                original.retain_cleanup(error.clone());
            } else {
                slot.failure = Some(error);
            }
        }
        self.connections[index].phase = Phase::Retiring;
        lock(&self.work.shared.state).connections[index].closing = true;
        core.changed();
    }
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        let observed = if clock_available { core.clock.now() } else { now };
        if self.outcome.is_none() {
            if let Some(reason) = crate::task::stop_reason_at(&self.context, observed) {
                self.outcome = Some(crate::task::stopped_outcome(reason));
            } else if lock(&self.work.shared.state).stopped {
                self.outcome = Some(JobOutcome::Success(self.report));
            } else if (!self.work.shared.server || lock(&self.work.shared.state).owner.is_none())
                && observed >= self.work.shared.original_preparation
            {
                self.outcome = Some(JobOutcome::TimedOut);
            }
        }
        if self.outcome.is_none()
            && let Err(error) = self.advance(core, observed)
        {
            self.error(core, error);
        }
        if self.outcome.is_some() {
            self.retirement(core, observed, clock_available);
        }
    }
    fn publish_deadline(&mut self, core: &Core) -> Result<(), EntryTransportError> {
        let s = lock(&self.work.shared.state);
        let mut earliest = if s.owner.is_none() || !self.work.shared.server {
            Some(self.work.shared.original_preparation)
        } else {
            None
        };
        for (index, io) in self.connections.iter().enumerate() {
            if io.phase == Phase::Retiring {
                continue;
            }
            for deadline in
                [io.pending.and_then(|io| io.deadline), s.connections[index].until, io.retry_at]
                    .into_iter()
                    .flatten()
            {
                earliest = Some(earliest.map_or(deadline, |previous| previous.min(deadline)));
            }
        }
        drop(s);
        let mut state = lock(&core.state);
        let slots = state
            .jobs
            .get_mut(&self.context.job_id())
            .and_then(|record| record.native_deadlines.as_mut())
            .ok_or(EntryTransportError::Closed)?;
        let changed = self.schedule.publish(slots, earliest)?;
        drop(state);
        if changed {
            core.changed();
        }
        Ok(())
    }
}
