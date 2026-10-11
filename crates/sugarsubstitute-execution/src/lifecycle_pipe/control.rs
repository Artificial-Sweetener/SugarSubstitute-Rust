// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Commit fixed commands and deadlines under Core-before-session lock ordering.
use super::*;
use crate::task::ErasedJob;
use sugarsubstitute_contracts::execution::{CancelControl, JobOutcome, JobPhase};

/// Retain a transport session and its actual checked terminal result.
pub struct LifecyclePipeSession {
    /// Fixed commands, bootstrap transfer and bounded actual observations.
    pub control: LifecyclePipeControl,
    /// Terminal follows actual I/O settlement, callback quiescence and handle closure.
    pub completion: OwnedJob<LifecyclePipeReport, LifecyclePipeFailure>,
}
/// Queue closed transport operations without raw handles, wait or scheduler access.
pub struct LifecyclePipeControl {
    pub(crate) shared: Arc<Shared>,
    pub(crate) job: Arc<Job<LifecyclePipeReport, LifecyclePipeFailure>>,
}
impl LifecyclePipeControl {
    /// Return the immutable admitted handshake deadline in this executor's epoch.
    /// Authentication clears its timer slot, without changing this original fact.
    pub fn original_deadline(&self) -> Moment {
        self.shared.original_deadline
    }

    pub(crate) fn commit<R>(
        &self,
        change: impl FnOnce(&mut State, &mut Deadlines, Moment) -> Result<R, LifecyclePipeError>,
    ) -> Result<R, LifecyclePipeError> {
        commit(&self.shared, &self.job, change)
    }
    /// Transfer the single read-only bootstrap launch pin after actual native preparation.
    /// Keep it through CreateProcess and attribute deletion; taking it twice rejects.
    pub fn take_bootstrap(&self) -> Result<Option<LifecyclePipeBootstrap>, LifecyclePipeError> {
        self.commit(|s, _, _| {
            if s.transferred {
                return Err(LifecyclePipeError::AlreadyTransferred);
            }
            if !s.prepared || s.inherited == 0 {
                return Ok(None);
            }
            s.transferred = true;
            s.pin = true;
            Ok(Some(LifecyclePipeBootstrap { shared: self.shared.clone() }))
        })
    }
    /// Return immutable run facts only after the worker validated its native bootstrap.
    /// This is encoder metadata, not peer authentication or resource authority.
    pub fn prepared_run(&self) -> Option<RunKey> {
        let s = lock(&self.shared.state);
        s.prepared.then_some(s.run).flatten()
    }
    /// Take one fixed actual observation; unknown application bytes retain their guard.
    pub fn take_observation(&self) -> Option<LifecyclePipeObservation> {
        lock(&self.shared.state).observations.pop_front()
    }
    /// Obtain the preclaimed output allocation; no new buffer allocation occurs here.
    pub fn take_write_buffer(
        &self,
    ) -> Result<Option<LifecyclePipeWriteBuffer>, LifecyclePipeError> {
        self.commit(|s, _, _| {
            let Some(buffer) = s.write_buffer.take() else {
                return Ok(None);
            };
            s.loans += 1;
            Ok(Some(LifecyclePipeWriteBuffer { buffer: Some(buffer), shared: self.shared.clone() }))
        })
    }
    /// Queue one bounded message read with its original composing-clock deadline.
    /// A held frame or pending read returns Busy; IDs never reuse or wrap.
    pub fn read(
        &self,
        operation: LifecyclePipeOperationId,
        original_deadline: Moment,
    ) -> Result<(), LifecyclePipeError> {
        self.commit(|s, slots, now| {
            validate(s, operation, original_deadline, now)?;
            if s.read_busy || s.read_buffer.is_none() || !s.room(2) {
                return Err(LifecyclePipeError::Busy);
            }
            if !slots.arm(Role::Read, operation.get(), original_deadline) {
                return Err(LifecyclePipeError::Busy);
            }
            s.latest = operation.get();
            s.read_busy = true;
            s.read = Some(Command { operation, deadline: original_deadline });
            Ok(())
        })
    }
    /// Transfer an encoded message without a second allocation or capacity decision.
    /// Rejection returns the original buffer, preserving its charge and ownership.
    pub fn write(
        &self,
        operation: LifecyclePipeOperationId,
        original_deadline: Moment,
        mut buffer: LifecyclePipeWriteBuffer,
        encoded_length: usize,
    ) -> Result<(), LifecyclePipeWriteRejection> {
        let result = self.commit(|s, slots, now| {
            validate(s, operation, original_deadline, now)?;
            if !Arc::ptr_eq(&buffer.shared, &self.shared)
                || buffer.buffer.is_none()
                || encoded_length == 0
                || encoded_length > MAX_LIFECYCLE_MESSAGE_BYTES
            {
                return Err(LifecyclePipeError::InvalidMessage);
            }
            if s.write_busy || !s.room(1) {
                return Err(LifecyclePipeError::Busy);
            }
            if !slots.arm(Role::Write, operation.get(), original_deadline) {
                return Err(LifecyclePipeError::Busy);
            }
            let storage = buffer.buffer.take().ok_or(LifecyclePipeError::InvalidMessage)?;
            s.loans -= 1;
            s.latest = operation.get();
            s.write_busy = true;
            s.write = Some(Write {
                command: Command { operation, deadline: original_deadline },
                buffer: storage,
                length: encoded_length,
            });
            Ok(())
        });
        result.map_err(|error| LifecyclePipeWriteRejection { error, buffer })
    }
    /// Queue one capability comparison after the bounded Hello read has completed.
    /// This is a trusted native adapter operation, not a decoded authentication fact.
    /// The supplied original deadline cannot extend the admitted handshake deadline.
    pub fn verify_capability(
        &self,
        candidate: [u8; 32],
        original_deadline: Moment,
    ) -> Result<(), LifecyclePipeError> {
        self.commit(|s, slots, now| {
            if !s.connected {
                return Err(LifecyclePipeError::NotReady);
            }
            if s.authenticated || s.auth_busy || !s.room(1) {
                return Err(LifecyclePipeError::Busy);
            }
            if original_deadline <= now {
                return Err(LifecyclePipeError::DeadlineExpired);
            }
            if slots.handshake_deadline().is_none_or(|admitted| original_deadline > admitted) {
                return Err(LifecyclePipeError::InvalidDeadline);
            }
            if !slots.shorten_handshake(original_deadline) {
                return Err(LifecyclePipeError::DeadlineExpired);
            }
            s.auth_busy = true;
            s.authenticate = Some(Authentication { candidate, deadline: original_deadline });
            Ok(())
        })
    }
    /// Transfer the child's capability once after checked read-only bootstrap validation.
    /// Native protocol code may encode Hello; JavaScript and backend code must not receive it.
    pub fn take_client_capability(
        &self,
    ) -> Result<Option<LifecycleClientCapability>, LifecyclePipeError> {
        self.commit(|s, _, _| Ok(s.capability.take().map(LifecycleClientCapability)))
    }
    /// Close command admission and request actual cancellation/retirement.
    /// No method treats a requested cancel as pipe, process or family settlement.
    pub fn close(&self) {
        {
            let mut s = lock(&self.shared.state);
            s.stopped = true;
        }
        self.job.cancel();
        self.shared.changed();
    }
    pub(crate) fn bind_input(&self, input: PeerInput) {
        {
            let mut s = lock(&self.shared.state);
            s.peer = Some(input);
            s.enrolled = true;
            s.input_pin = false;
        }
        self.shared.changed();
    }
}
impl Drop for LifecyclePipeControl {
    fn drop(&mut self) {
        self.close();
    }
}
fn validate(
    s: &State,
    operation: LifecyclePipeOperationId,
    deadline: Moment,
    now: Moment,
) -> Result<(), LifecyclePipeError> {
    if !s.connected {
        return Err(LifecyclePipeError::NotReady);
    }
    if operation.get() <= s.latest {
        return Err(LifecyclePipeError::InvalidOperation);
    }
    if deadline <= now {
        return Err(LifecyclePipeError::DeadlineExpired);
    }
    Ok(())
}
/// Preserve a rejected encoded buffer without destroying or overwriting its contents.
pub struct LifecyclePipeWriteRejection {
    /// Typed rejection, without message contents.
    pub error: LifecyclePipeError,
    /// The original owned allocation, available for an explicitly chosen later operation.
    pub buffer: LifecyclePipeWriteBuffer,
}
impl std::fmt::Debug for LifecyclePipeWriteRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LifecyclePipeWriteRejection")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
/// Pin one immutable read-only bootstrap handle through native launch attributes.
/// Drop only releases the pin and wakes its execution owner; it performs no native close.
pub struct LifecyclePipeBootstrap {
    pub(crate) shared: Arc<Shared>,
}
impl Drop for LifecyclePipeBootstrap {
    fn drop(&mut self) {
        {
            lock(&self.shared.state).pin = false;
        }
        self.shared.changed();
    }
}
impl LifecyclePipeBootstrap {
    pub(crate) fn handle(&self) -> usize {
        lock(&self.shared.state).inherited
    }
}
impl LifecyclePipeBootstrap {
    /// Return immutable deadline facts to pass unchanged to the isolated child role.
    pub fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        lock(&self.shared.state).deadline
    }
}

pub(crate) fn retain_failure(
    outcome: &mut Option<JobOutcome<LifecyclePipeReport, LifecyclePipeFailure>>,
    mut error: LifecyclePipeFailure,
) {
    match outcome {
        Some(JobOutcome::Failure(first)) => {
            first.identity_unrestored |= error.identity_unrestored;
            first.cause.retain_cleanup(error.cause);
        }
        Some(JobOutcome::Panicked(cause)) => {
            error.cause.retain_panic(cause);
            *outcome = Some(JobOutcome::Failure(error));
        }
        outcome => *outcome = Some(JobOutcome::Failure(error)),
    }
}

pub(super) fn commit<R>(
    shared: &Arc<Shared>,
    job: &Arc<Job<LifecyclePipeReport, LifecyclePipeFailure>>,
    change: impl FnOnce(&mut State, &mut Deadlines, Moment) -> Result<R, LifecyclePipeError>,
) -> Result<R, LifecyclePipeError> {
    let core = shared.core.upgrade().ok_or(LifecyclePipeError::Closed)?;
    let now = core.clock.now();
    let result = {
        let mut state = lock(&core.state);
        let stopping = state.stopping;
        let record = state.jobs.get_mut(&shared.id).ok_or(LifecyclePipeError::Closed)?;
        let mut pipe = lock(&shared.state);
        if stopping
            || record.retiring
            || matches!(record.phase, JobPhase::Terminal(_))
            || pipe.stopped
            || job.cancel.is_requested()
        {
            return Err(LifecyclePipeError::Closed);
        }
        let slots = record.native_deadlines.as_mut().ok_or(LifecyclePipeError::Closed)?;
        change(&mut pipe, slots, now)
    };
    if result.is_ok() {
        core.changed();
    }
    result
}
