// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Commit actual command completions only after callback quiescence and fresh deadlines.
use super::*;
impl ActiveState {
    pub(super) fn protocol(&mut self, core: &Core) -> Result<(), LifecyclePipeFailure> {
        if self.stop(core, core.clock.now()) {
            return Ok(());
        }
        if !self.connected {
            let Some(lease) = &mut self.lease else {
                return Err(failure("pipe_missing_lease"));
            };
            let progress = if self.connecting {
                if self.signals[0].completed() {
                    lease.poll(Role::Handshake)?
                } else {
                    Progress::Pending
                }
            } else {
                self.connecting = true;
                match lease.connect(Issue {
                    core,
                    shared: &self.work.shared,
                    context: &self.context,
                    activity: Activity::Connect,
                })? {
                    Dispatch::Issued(progress) => progress,
                    Dispatch::Stopped | Dispatch::Revoked => {
                        self.outcome = Some(JobOutcome::Cancelled);
                        return Ok(());
                    }
                    Dispatch::Expired => {
                        self.outcome = Some(JobOutcome::TimedOut);
                        return Ok(());
                    }
                }
            };
            if let Progress::Complete(_) = progress {
                lease.quiesce(Role::Handshake)?;
                self.connected = true;
                let mut s = lock(&self.work.shared.state);
                s.connected = true;
                s.observations.push_back(Observation::Connected);
                drop(s);
                self.publish(core);
            }
        }
        if !self.connected || self.stop(core, core.clock.now()) {
            return Ok(());
        }
        if self.read.is_none() {
            let next = {
                let state = lock(&core.state);
                let mut s = lock(&self.work.shared.state);
                if state.stopping || s.stopped {
                    None
                } else {
                    s.read
                        .take()
                        .and_then(|command| s.read_buffer.take().map(|buffer| (command, buffer)))
                }
            };
            if let Some((command, buffer)) = next {
                if let ReadDeadline::Original(until) = command.deadline
                    && until <= core.clock.now()
                {
                    self.read = Some((command, buffer));
                    self.outcome = Some(JobOutcome::TimedOut);
                    return Ok(());
                }
                self.signals[1].reset();
                let operation = command.operation;
                self.read = Some((command, buffer));
                let (_, buffer) = self.read.as_mut().ok_or_else(|| failure("pipe_read_owner"))?;
                // The pinned buffer is durable before any native issue or private panic.
                let progress =
                    self.lease.as_mut().ok_or_else(|| failure("pipe_missing_lease"))?.read(
                        &mut buffer.bytes,
                        Issue {
                            core,
                            shared: &self.work.shared,
                            context: &self.context,
                            activity: Activity::Read(&command),
                        },
                    )?;
                let progress = match progress {
                    Dispatch::Issued(progress) => progress,
                    Dispatch::Stopped | Dispatch::Revoked => {
                        self.outcome = Some(JobOutcome::Cancelled);
                        return Ok(());
                    }
                    Dispatch::Expired => {
                        self.outcome = Some(JobOutcome::TimedOut);
                        return Ok(());
                    }
                };
                if progress == Progress::Pending {
                    lock(&self.work.shared.state)
                        .observations
                        .push_back(Observation::ReadPending { operation });
                    self.publish(core);
                }
                self.read_progress(core, progress)?;
            }
        } else if self.signals[1].completed() {
            let result = self
                .lease
                .as_mut()
                .ok_or_else(|| failure("pipe_missing_lease"))?
                .poll(Role::Read)?;
            self.read_progress(core, result)?;
        }
        if self.stop(core, core.clock.now()) {
            return Ok(());
        }
        if self.write.is_none() {
            let next = {
                let state = lock(&core.state);
                let mut s = lock(&self.work.shared.state);
                if state.stopping || s.stopped { None } else { s.write.take() }
            };
            if let Some(write) = next {
                self.signals[2].reset();
                self.write = Some(write);
                let write = self.write.as_ref().ok_or_else(|| failure("pipe_write_owner"))?;
                // Any unwind after issue leaves immutable storage in this retained Active.
                let progress =
                    self.lease.as_mut().ok_or_else(|| failure("pipe_missing_lease"))?.write(
                        &write.buffer.bytes[..write.length],
                        Issue {
                            core,
                            shared: &self.work.shared,
                            context: &self.context,
                            activity: Activity::Write(&write.command),
                        },
                    )?;
                match progress {
                    Dispatch::Issued(progress) => {
                        self.publish(core);
                        self.write_progress(core, progress)?;
                    }
                    Dispatch::Revoked => self.write_revoked(core)?,
                    Dispatch::Stopped => {
                        self.outcome = Some(JobOutcome::Cancelled);
                    }
                    Dispatch::Expired => {
                        self.outcome = Some(JobOutcome::TimedOut);
                    }
                }
            }
        } else if self.signals[2].completed() {
            let result = self
                .lease
                .as_mut()
                .ok_or_else(|| failure("pipe_missing_lease"))?
                .poll(Role::Write)?;
            self.write_progress(core, result)?;
        }
        if self.stop(core, core.clock.now()) {
            return Ok(());
        }
        let authentication = { lock(&self.work.shared.state).authenticate.take() };
        if let Some(authentication) = authentication {
            if authentication.deadline <= core.clock.now() {
                self.outcome = Some(JobOutcome::TimedOut);
                return Ok(());
            }
            let peer = self
                .lease
                .as_mut()
                .ok_or_else(|| failure("pipe_missing_lease"))?
                .authenticate(&authentication.candidate)?;
            #[cfg(windows)]
            let captured = self.lease.as_mut().and_then(|lease| lease.take_captured_startup());
            // Adopt both outputs before the fresh stop/eligibility decision. On veto,
            // shutdown returns their loans outside these fences before native closure.
            #[cfg(windows)]
            if let Some(captured) = captured {
                lock(&self.work.shared.state).captured_startup = Some(captured);
            }
            if self.stop(core, core.clock.now()) {
                return Ok(());
            }
            {
                let mut state = lock(&core.state);
                let mut s = lock(&self.work.shared.state);
                if s.stopped || state.stopping {
                    return Ok(());
                }
                if let Some(slots) = state
                    .jobs
                    .get_mut(&self.context.job_id())
                    .and_then(|r| r.native_deadlines.as_mut())
                {
                    slots.clear(Role::Handshake, 0);
                }
                if s.binding != Some(peer.binding) {
                    return Err(failure("pipe_authenticated_binding"));
                }
                s.auth_busy = false;
                s.authenticated = true;
                s.observations.push_back(Observation::Authenticated(peer));
            }
            core.changed();
            self.publish(core);
        }
        Ok(())
    }
    fn read_progress(
        &mut self,
        core: &Core,
        progress: Progress,
    ) -> Result<(), LifecyclePipeFailure> {
        let Progress::Complete(bytes) = progress else { return Ok(()) };
        if bytes == 0 || bytes as usize > MAX_LIFECYCLE_MESSAGE_BYTES {
            return Err(failure("pipe_message_bound"));
        }
        let lease = self.lease.as_mut().ok_or_else(|| failure("pipe_missing_lease"))?;
        lease.quiesce(Role::Read)?;
        lease.verify_message()?;
        if self.stop(core, core.clock.now()) {
            return Ok(());
        }
        let Some((command, buffer)) = self.read.take() else {
            return Err(failure("pipe_read_owner"));
        };
        {
            let mut state = lock(&core.state);
            let mut s = lock(&self.work.shared.state);
            if matches!(command.deadline, ReadDeadline::Original(_))
                && let Some(slots) = state
                    .jobs
                    .get_mut(&self.context.job_id())
                    .and_then(|r| r.native_deadlines.as_mut())
            {
                slots.clear(Role::Read, command.operation.get());
            }
            s.read_busy = false;
            s.received += 1;
            s.loans += 1;
            s.observations.push_back(Observation::Received {
                operation: command.operation,
                frame: ReceivedLifecycleFrame {
                    buffer: Some(buffer),
                    length: bytes as usize,
                    shared: self.work.shared.clone(),
                },
            });
        }
        core.changed();
        self.publish(core);
        Ok(())
    }
    fn write_progress(
        &mut self,
        core: &Core,
        progress: Progress,
    ) -> Result<(), LifecyclePipeFailure> {
        let Progress::Complete(bytes) = progress else { return Ok(()) };
        if self.write.as_ref().is_none_or(|write| bytes as usize != write.length) {
            return Err(failure("pipe_partial_write"));
        }
        self.lease.as_mut().ok_or_else(|| failure("pipe_missing_lease"))?.quiesce(Role::Write)?;
        if self.stop(core, core.clock.now()) {
            return Ok(());
        }
        let Some(write) = self.write.take() else {
            return Err(failure("pipe_write_owner"));
        };
        // Normal publication consumes this operation's native completion evidence.
        // A stopped operation instead keeps it for checked retirement publication.
        if let Some(lease) = &mut self.lease {
            let _ = lease.take_completed_write();
        }
        {
            let mut state = lock(&core.state);
            let mut s = lock(&self.work.shared.state);
            if let Some(slots) =
                state.jobs.get_mut(&self.context.job_id()).and_then(|r| r.native_deadlines.as_mut())
            {
                slots.clear(Role::Write, write.command.operation.get());
            }
            s.write_busy = false;
            s.write_slots = 0;
            s.written += 1;
            s.write_buffer = Some(write.buffer);
            s.observations
                .push_back(Observation::Written { operation: write.command.operation, bytes });
        }
        core.changed();
        self.publish(core);
        Ok(())
    }
}
