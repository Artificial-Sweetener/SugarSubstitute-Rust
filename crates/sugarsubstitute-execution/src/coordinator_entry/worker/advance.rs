// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance closed phases without moving live native buffers into fallible results.
use super::*;
impl Active {
    pub(super) fn advance(&mut self, core: &Core, now: Moment) -> Result<(), EntryTransportError> {
        if !self.initialized {
            self.allocate(core)?;
        }
        if !self.prepared {
            self.lease.as_mut().ok_or(EntryTransportError::InvalidPhase)?.prepare(
                &self.signals,
                &self.context,
                self.work.shared.original_preparation,
            )?;
            self.prepared = true;
            lock(&self.work.shared.state).prepared = true;
            self.emit(core, None, EntryTransportObservation::Prepared)?;
        }
        let activation = { lock(&self.work.shared.state).activation.take() };
        if let Some(owner) = activation {
            if now >= self.work.shared.original_preparation {
                return Err(EntryTransportError::InvalidDeadline);
            }
            lock(&self.work.shared.state).owner = Some(owner);
            self.emit(core, None, EntryTransportObservation::ServerActivated { owner })?;
        }
        self.commands(core, now)?;
        for index in 0..self.connections.len() {
            let (closing, expired, returned) = {
                let mut state = lock(&self.work.shared.state);
                let entry = &mut state.connections[index];
                (
                    entry.closing,
                    entry.until.is_some_and(|until| now >= until),
                    entry.returned.take(),
                )
            };
            if let Some(buffer) = returned {
                self.connections[index].read = Some(buffer);
            }
            if closing {
                self.close_connection(core, index, None);
            } else if expired
                || self.connections[index]
                    .pending
                    .is_some_and(|io| io.deadline.is_some_and(|deadline| now >= deadline))
            {
                self.expire_probe(core, index, now)?;
            }
            if self.connections[index].phase == Phase::Retiring {
                self.retire_connection(core, index)?;
                continue;
            }
            // A bounded pass handles synchronous transitions without polling or busy loops.
            for _ in 0..8 {
                if let Some(io) = self.connections[index].pending {
                    let result = self.lease()?.poll(index, io.role);
                    match result {
                        Ok(Progress::Pending) => break,
                        Ok(Progress::Complete(bytes)) => {
                            if let Err(error) = self.complete(core, index, io, bytes) {
                                self.close_connection(core, index, Some(error));
                                break;
                            }
                        }
                        Err(error) => {
                            self.close_connection(core, index, Some(error));
                            break;
                        }
                    }
                } else if let Err(error) = self.step(core, index, now) {
                    self.close_connection(core, index, Some(error));
                    break;
                } else if self.connections[index].pending.is_some()
                    || matches!(
                        self.connections[index].phase,
                        Phase::AwaitRoot | Phase::Retiring | Phase::Available
                    )
                {
                    break;
                }
            }
        }
        self.publish_deadline(core)
    }
    /// Retire the expired native operation before a bounded same-source reprobe.
    /// The original setup cutoff and staged request remain unchanged; checked
    /// connection retirement must finish before the existing reuse path can run.
    fn expire_probe(
        &mut self,
        core: &Core,
        index: usize,
        now: Moment,
    ) -> Result<(), EntryTransportError> {
        if !self.work.shared.server {
            if now < self.work.shared.original_preparation {
                self.connections[index].retry_at =
                    Some(schedule::probe(now, self.work.shared.original_preparation)?);
            } else {
                self.outcome.get_or_insert(JobOutcome::TimedOut);
            }
        }
        self.close_connection(core, index, None);
        Ok(())
    }
    fn step(&mut self, core: &Core, index: usize, now: Moment) -> Result<(), EntryTransportError> {
        match self.connections[index].phase {
            Phase::Available => self.connect(core, index, now),
            Phase::Hello | Phase::Challenge | Phase::Launch | Phase::Outcome => {
                self.read_frame(core, index, now)
            }
            Phase::AwaitRoot => self.cached_reply(core, index, now),
            Phase::Connecting | Phase::Retiring => Ok(()),
        }
    }
    fn connect(
        &mut self,
        core: &Core,
        index: usize,
        now: Moment,
    ) -> Result<(), EntryTransportError> {
        if self.connections[index].retry_at.is_some_and(|at| now < at) {
            return Ok(());
        }
        self.connections[index].retry_at = None;
        let id = {
            let mut state = lock(&self.work.shared.state);
            if state.connections[index].id.is_some() {
                return Err(EntryTransportError::InvalidPhase);
            }
            // Retained observations from old lifetimes keep their credits; overload defers accept.
            match state.reserve(index, 4) {
                Ok(()) => {}
                Err(EntryTransportError::Busy) => return Ok(()),
                Err(error) => return Err(error),
            }
            state.next_connection = state
                .next_connection
                .checked_add(1)
                .ok_or(EntryTransportError::InvalidOperation)?;
            let id = EntryConnectionId::new(state.next_connection)
                .map_err(EntryTransportError::Shape)?;
            let entry = &mut state.connections[index];
            entry.id = Some(id);
            entry.closing = false;
            id
        };
        self.connections[index].phase = Phase::Connecting;
        let io = Io {
            role: Role::Handshake,
            length: 0,
            next: if self.work.shared.server { Phase::Hello } else { Phase::Challenge },
            deadline: if self.work.shared.server {
                None
            } else {
                Some(schedule::probe(now, self.work.shared.original_preparation)?)
            },
            operation: None,
        };
        self.connections[index].pending = Some(io);
        let issue = NativeIssue {
            core,
            shared: &self.work.shared,
            context: &self.context,
            index,
            connection: id,
            role: io.role,
            deadline: io.deadline,
        };
        let lease = self.lease.as_mut().ok_or(EntryTransportError::InvalidPhase)?;
        let connected = lease.connect(index, issue);
        if let Err(error @ EntryTransportError::EndpointAbsent(_)) = &connected
            && !self.work.shared.server
            && !error.contains_owner_panic()
        {
            // Retire this exact unaccepted attempt before an original-budget retry.
            // Missing endpoint facts never authenticate a peer or grant election authority.
            self.connections[index].failure = Some(error.clone());
            return self.expire_probe(core, index, core.clock.now());
        }
        match connected? {
            IssueDispatch::Issued(Progress::Pending) => Ok(()),
            IssueDispatch::Issued(Progress::Complete(bytes)) => {
                self.complete(core, index, io, bytes)
            }
            IssueDispatch::Closed => {
                self.close_connection(core, index, None);
                Ok(())
            }
            IssueDispatch::Expired => self.expire_probe(core, index, core.clock.now()),
        }
    }
    fn read_frame(
        &mut self,
        core: &Core,
        index: usize,
        now: Moment,
    ) -> Result<(), EntryTransportError> {
        if lock(&self.work.shared.state).connections[index].loan {
            return Ok(());
        }
        let ancestor = { lock(&self.work.shared.state).connections[index].until }
            .ok_or(EntryTransportError::InvalidPhase)?;
        let command = { lock(&self.work.shared.state).connections[index].read.take() };
        let deadline = command
            .map_or_else(|| schedule::probe(now, ancestor), |command| Ok(command.deadline))?;
        if deadline <= core.clock.now() {
            return Err(EntryTransportError::InvalidDeadline);
        }
        let io = Io {
            role: Role::Read,
            length: 0,
            next: self.connections[index].phase,
            deadline: Some(deadline),
            operation: command.map(|command| command.operation),
        };
        self.connections[index].pending = Some(io);
        lock(&self.work.shared.state).connections[index].reading = true;
        let lease = self.lease.as_mut().ok_or(EntryTransportError::InvalidPhase)?;
        let buffer = self.connections[index].read.as_mut().ok_or(EntryTransportError::Busy)?;
        let connection = lock(&self.work.shared.state).connections[index]
            .id
            .ok_or(EntryTransportError::InvalidPhase)?;
        let issue = NativeIssue {
            core,
            shared: &self.work.shared,
            context: &self.context,
            index,
            connection,
            role: io.role,
            deadline: io.deadline,
        };
        match lease.read(index, &mut buffer.bytes, issue)? {
            IssueDispatch::Issued(Progress::Pending) => Ok(()),
            IssueDispatch::Issued(Progress::Complete(bytes)) => {
                self.complete(core, index, io, bytes)
            }
            IssueDispatch::Closed => {
                self.close_connection(core, index, None);
                Ok(())
            }
            IssueDispatch::Expired => self.expire_probe(core, index, core.clock.now()),
        }
    }
    pub(super) fn write_frame(
        &mut self,
        core: &Core,
        index: usize,
        length: usize,
        next: Phase,
        deadline: Moment,
        operation: Option<EntryOperationId>,
    ) -> Result<(), EntryTransportError> {
        if deadline <= core.clock.now() {
            return Err(EntryTransportError::InvalidDeadline);
        }
        let io = Io { role: Role::Write, length, next, deadline: Some(deadline), operation };
        self.connections[index].pending = Some(io);
        lock(&self.work.shared.state).connections[index].writing = true;
        let lease = self.lease.as_mut().ok_or(EntryTransportError::InvalidPhase)?;
        let bytes = if !self.work.shared.server
            && self.connections[index].phase == Phase::Connecting
        {
            &self.client.as_ref().ok_or(EntryTransportError::InvalidPhase)?.hello[..]
        } else {
            &self.connections[index].write.as_ref().ok_or(EntryTransportError::InvalidPhase)?.bytes
                [..length]
        };
        let connection = lock(&self.work.shared.state).connections[index]
            .id
            .ok_or(EntryTransportError::InvalidPhase)?;
        let issue = NativeIssue {
            core,
            shared: &self.work.shared,
            context: &self.context,
            index,
            connection,
            role: Role::Write,
            deadline: Some(deadline),
        };
        match lease.write(index, bytes, issue)? {
            IssueDispatch::Issued(Progress::Pending) => Ok(()),
            IssueDispatch::Issued(Progress::Complete(bytes)) => {
                self.complete(core, index, io, bytes)
            }
            IssueDispatch::Closed => {
                self.close_connection(core, index, None);
                Ok(())
            }
            IssueDispatch::Expired => self.expire_probe(core, index, core.clock.now()),
        }
    }
    fn complete(
        &mut self,
        core: &Core,
        index: usize,
        io: Io,
        bytes: u32,
    ) -> Result<(), EntryTransportError> {
        self.lease()?.quiesce(index, io.role)?;
        // The OVERLAPPED owner is quiescent before clearing its adopted buffer operation.
        self.connections[index].pending = None;
        let now = core.clock.now();
        if io.deadline.is_some_and(|deadline| now >= deadline) {
            if !self.work.shared.server {
                // Checked quiescence returned after the finite probe. Discard
                // these late bytes before any protocol outcome is decoded.
                return self.expire_probe(core, index, now);
            }
            return Err(EntryTransportError::InvalidDeadline);
        }
        match io.role {
            Role::Handshake => {
                let peer = self.verified_peer(index, false)?;
                {
                    let mut state = lock(&self.work.shared.state);
                    let serving = state.owner.is_some();
                    let entry = &mut state.connections[index];
                    entry.peer = Some(peer);
                    entry.until = Some(if self.work.shared.server && serving {
                        now.checked_add(std::time::Duration::from_secs(2)).map_err(|cause| {
                            EntryTransportError::Clock(NativeObservationError::new(
                                "entry_unknown_source_probe",
                                cause,
                            ))
                        })?
                    } else {
                        self.work.shared.original_preparation
                    });
                }
                self.emit(
                    core,
                    Some(index),
                    EntryTransportObservation::ConnectionVerified {
                        connection: peer.connection(),
                        peer,
                    },
                )?;
                if self.work.shared.server {
                    self.connections[index].phase = Phase::Hello;
                } else {
                    let deadline = schedule::probe(now, self.work.shared.original_preparation)?;
                    self.write_frame(
                        core,
                        index,
                        protocol::HEADER_BYTES,
                        Phase::Challenge,
                        deadline,
                        None,
                    )?;
                }
            }
            Role::Read => {
                lock(&self.work.shared.state).connections[index].reading = false;
                if io.operation.is_some() {
                    let mut state = lock(&self.work.shared.state);
                    let slot = &mut state.connections[index];
                    if slot.reserved != 0 {
                        slot.reserved -= 1;
                    }
                }
                self.received(core, index, bytes as usize, now)?;
            }
            Role::Write => {
                lock(&self.work.shared.state).connections[index].writing = false;
                if bytes as usize != io.length {
                    return Err(EntryTransportError::Malformed);
                }
                self.report.completed_writes += 1;
                self.connections[index].phase = io.next;
                if let Some(operation) = io.operation {
                    let connection = lock(&self.work.shared.state).connections[index]
                        .id
                        .ok_or(EntryTransportError::InvalidPhase)?;
                    self.emit(
                        core,
                        Some(index),
                        EntryTransportObservation::Written { connection, operation, bytes },
                    )?;
                }
                if io.next == Phase::Retiring {
                    self.close_connection(core, index, None);
                }
            }
        }
        Ok(())
    }
}
