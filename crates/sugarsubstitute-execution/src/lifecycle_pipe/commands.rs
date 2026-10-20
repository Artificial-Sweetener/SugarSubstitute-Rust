// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own all fixed pipe commands under one admission and issue policy.
use super::*;
use sugarsubstitute_contracts::execution::{JobOutcome, JobPhase};
pub(crate) struct Control {
    pub(crate) owner: Owner,
    pub(crate) shared: SharedRef,
}
impl Control {
    /// Return the immutable admitted handshake deadline in this executor's epoch.
    /// Authentication clears its timer slot, without changing this original fact.
    pub(crate) fn original_deadline(&self) -> Moment {
        self.shared.original_deadline
    }

    pub(crate) fn commit<R>(
        &self,
        change: impl FnOnce(&mut State, &mut Deadlines, Moment) -> Result<R, LifecyclePipeError>,
    ) -> Result<R, LifecyclePipeError> {
        commit(&self.shared, &self.owner, change)
    }
    /// Transfer the single read-only bootstrap launch pin after actual native preparation.
    /// Keep it through CreateProcess and attribute deletion; taking it twice rejects.
    pub(crate) fn take_bootstrap(
        &self,
    ) -> Result<Option<LifecyclePipeBootstrap>, LifecyclePipeError> {
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
    pub(crate) fn binding(&self) -> Option<PeerBinding> {
        let s = lock(&self.shared.state);
        s.prepared.then_some(s.binding).flatten()
    }
    /// Take one fixed actual observation; unknown application bytes retain their guard.
    pub(crate) fn take_observation(&self) -> Option<Observation> {
        lock(&self.shared.state).observations.pop_front()
    }
    /// Obtain the preclaimed output allocation; no new buffer allocation occurs here.
    pub(crate) fn take_write_buffer(
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
    pub(crate) fn read(
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
            s.read = Some(ReadCommand {
                operation,
                deadline: ReadDeadline::Original(original_deadline),
            });
            Ok(())
        })
    }
    /// Queue one passive receive using this authenticated session's preclaimed read buffer.
    ///
    /// Idle receive has no command timer. Received bytes remain untrusted; their typed
    /// consumer must validate each request's original header deadline and identities.
    /// Cancellation, original finite operations and peer closure still retire the same
    /// native read, callback and buffer before terminal publication.
    /// # Errors
    /// Rejects unauthenticated/noncurrent sessions, reused identities, occupied read
    /// storage and observation capacity. Rejection preserves timers and storage.
    pub(crate) fn read_passive(
        &self,
        operation: LifecyclePipeOperationId,
    ) -> Result<(), LifecyclePipeError> {
        self.commit(|s, slots, _| {
            if !s.authenticated || slots.handshake_deadline().is_some() {
                return Err(LifecyclePipeError::NotReady);
            }
            validate_operation(s, operation)?;
            if s.read_busy || s.read_buffer.is_none() || !s.room(2) {
                return Err(LifecyclePipeError::Busy);
            }
            s.latest = operation.get();
            s.read_busy = true;
            s.read = Some(ReadCommand { operation, deadline: ReadDeadline::Passive });
            Ok(())
        })
    }
    /// Transfer an encoded message without a second allocation or capacity decision.
    /// Rejection returns the original buffer, preserving its charge and ownership.
    /// The passive admission can veto this exact write before issue; dropping it does not revoke.
    /// WriteIssued reports actual acceptance separately from later native completion.
    pub(crate) fn write(
        &self,
        operation: LifecyclePipeOperationId,
        original_deadline: Moment,
        buffer: LifecyclePipeWriteBuffer,
        encoded_length: usize,
    ) -> Result<NativeWriteAdmission, LifecyclePipeWriteRejection> {
        self.write_command(operation, Some(original_deadline), buffer, encoded_length)
    }
    /// Queue only a canonical authenticated custody heartbeat under its active-time watchdog.
    pub(crate) fn write_heartbeat(
        &self,
        operation: LifecyclePipeOperationId,
        buffer: LifecyclePipeWriteBuffer,
    ) -> Result<NativeWriteAdmission, LifecyclePipeWriteRejection> {
        self.write_command(
            operation,
            None,
            buffer,
            sugarsubstitute_contracts::backend_custody::BACKEND_CUSTODY_FRAME_BYTES,
        )
    }
    /// Preserve the same buffer/admission owner for finite commands and typed heartbeats.
    fn write_command(
        &self,
        operation: LifecyclePipeOperationId,
        original_deadline: Option<Moment>,
        mut buffer: LifecyclePipeWriteBuffer,
        encoded_length: usize,
    ) -> Result<NativeWriteAdmission, LifecyclePipeWriteRejection> {
        let result = self.commit(|s, slots, now| {
            if let Some(until) = original_deadline {
                validate(s, operation, until, now)?;
            } else {
                validate_operation(s, operation)?;
                if !s.authenticated || slots.handshake_deadline().is_some() {
                    return Err(LifecyclePipeError::NotReady);
                }
                use sugarsubstitute_contracts::backend_custody::{
                    BackendCustodyCommand as Command, BackendCustodyFrame,
                    BackendCustodyReceipt as Receipt,
                };
                let bytes = buffer
                    .buffer
                    .as_ref()
                    .and_then(|buffer| buffer.bytes.get(..encoded_length))
                    .ok_or(LifecyclePipeError::InvalidMessage)?;
                let frame = BackendCustodyFrame::decode(bytes)
                    .map_err(|_| LifecyclePipeError::InvalidMessage)?;
                if Some(frame.key) != s.binding.and_then(PeerBinding::backend_custody)
                    || !matches!(
                        frame.command,
                        Command::Heartbeat | Command::Receipt(Receipt::Heartbeat)
                    )
                {
                    return Err(LifecyclePipeError::InvalidMessage);
                }
            }
            if !buffer.shared.same_allocation(&self.shared)
                || buffer.buffer.is_none()
                || encoded_length == 0
                || encoded_length > MAX_LIFECYCLE_MESSAGE_BYTES
            {
                return Err(LifecyclePipeError::InvalidMessage);
            }
            if s.write_busy || !s.room(2) {
                return Err(LifecyclePipeError::Busy);
            }
            if original_deadline
                .is_some_and(|until| !slots.arm(Role::Write, operation.get(), until))
            {
                return Err(LifecyclePipeError::Busy);
            }
            let storage = buffer.buffer.take().ok_or(LifecyclePipeError::InvalidMessage)?;
            s.loans -= 1;
            s.latest = operation.get();
            s.write_busy = true;
            s.write_slots = 2;
            s.write_admission =
                Some(admission::Admission { operation, phase: admission::Phase::Queued });
            s.write = Some(Write {
                command: Command { operation, deadline: original_deadline },
                buffer: storage,
                length: encoded_length,
            });
            Ok(())
        });
        result
            .map(|()| NativeWriteAdmission { shared: self.shared.clone(), operation })
            .map_err(|error| LifecyclePipeWriteRejection { error, buffer })
    }
    /// Queue one capability comparison after the bounded Hello read has completed.
    /// This is a trusted native adapter operation, not a decoded authentication fact.
    /// The supplied original deadline cannot extend the admitted handshake deadline.
    pub(crate) fn verify_capability(
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
    pub(crate) fn take_client_capability(
        &self,
    ) -> Result<Option<LifecycleClientCapability>, LifecyclePipeError> {
        self.commit(|s, _, _| Ok(s.capability.take().map(LifecycleClientCapability)))
    }
    /// Close command admission and request actual cancellation/retirement.
    /// No method treats a requested cancel as pipe, process or family settlement.
    pub(crate) fn close(&self) {
        {
            let mut s = lock(&self.shared.state);
            s.stopped = true;
        }
        self.owner.cancel();
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
fn validate(
    s: &State,
    operation: LifecyclePipeOperationId,
    deadline: Moment,
    now: Moment,
) -> Result<(), LifecyclePipeError> {
    validate_operation(s, operation)?;
    if deadline <= now {
        return Err(LifecyclePipeError::DeadlineExpired);
    }
    Ok(())
}
fn validate_operation(
    s: &State,
    operation: LifecyclePipeOperationId,
) -> Result<(), LifecyclePipeError> {
    if !s.connected {
        return Err(LifecyclePipeError::NotReady);
    }
    if operation.get() <= s.latest {
        return Err(LifecyclePipeError::InvalidOperation);
    }
    Ok(())
}
pub(crate) fn retain_failure<T>(
    outcome: &mut Option<JobOutcome<T, LifecyclePipeFailure>>,
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
    shared: &SharedRef,
    owner: &Owner,
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
            || owner.cancellation_requested()
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

impl Drop for Control {
    fn drop(&mut self) {
        self.close();
    }
}
