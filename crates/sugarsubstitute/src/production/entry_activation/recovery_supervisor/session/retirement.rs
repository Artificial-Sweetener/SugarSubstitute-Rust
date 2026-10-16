// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire only this control and observers under Root's independently original cleanup cutoff.
use super::framing::Stage;
use super::*;
impl RootRecoverySession {
    /// Latch Root's original capture-before-containment deadline once, without translating anew.
    /// The caller separately owns actual family stop and the original10/15s ShutdownBudget.
    pub(in crate::production::entry_activation::recovery_supervisor) fn begin_close(
        &mut self,
        counter: CounterDeadline,
        until: Moment,
    ) -> Result<(), SessionCode> {
        let state = self.state.as_mut().ok_or(SessionCode::Closed)?;
        if !Closing::latch(&mut state.closing, counter, until)? {
            return Ok(());
        }
        if let Some(worker) = &state.job {
            worker.job.cancel();
        }
        // A queued original write can be revoked; issued writes still require real completion.
        if let Some(admission) = &state.admission
            && let Some(close) = &mut state.closing
        {
            close._revocation = Some(admission.revoke());
        }
        Ok(())
    }
}
impl State {
    pub(super) fn progress_retirement(&mut self, now: Moment) {
        if self.pipe_retirement.is_none()
            && let Some(proof) = self.pipe.control().take_checked_retirement()
        {
            self.pipe_verified = proof.matches(self.pipe.control())
                && proof.key() == self.grant.key()
                && proof.scope_id() == self.pipe_scope
                && proof.spec() == self.pipe_spec
                && proof.execution_id() == self.execution;
            self.pipe_retirement = Some(proof);
            if !self.pipe_verified {
                self.record(SessionCause::Protocol(SessionCode::Binding));
            } else if self.closing.is_none() {
                self.record(SessionCause::Protocol(SessionCode::Terminal));
            }
        }
        let Some(close) = &self.closing else {
            return;
        };
        let both_exited = self.work.try_lock().is_ok_and(|work| {
            work.desktop.as_ref().and_then(RecoveryEnrolledDesktop::actual_exit).is_some()
                && work.handler.as_ref().and_then(RecoveryHandlerWatch::actual_exit).is_some()
        });
        let stop = now >= close.until
            || self.failures.first.is_some()
            || self.peer.is_none()
            || self.plan.stage != Stage::Active
            || (self.subordinate_retired.received() && both_exited);
        if !stop {
            return;
        }
        if let Some(close) = &mut self.closing {
            if !close.control_closed {
                self.pipe.control().close();
                close.control_closed = true;
            }
            if close.observers_cancelled || close.cancellation_failed {
                return;
            }
        }
        let shared = self.work.clone();
        let mut work = match shared.try_lock() {
            Ok(work) => work,
            Err(TryLockError::WouldBlock) => return,
            Err(TryLockError::Poisoned(poison)) => poison.into_inner(),
        };
        let result = cancel_observers(&mut work);
        drop(work);
        match result {
            Ok(()) => {
                if let Some(close) = &mut self.closing {
                    close.observers_cancelled = true;
                }
            }
            Err(cause) => {
                if let Some(close) = &mut self.closing {
                    close.cancellation_failed = true;
                }
                self.record(SessionCause::Registry(cause));
            }
        }
    }
    pub(super) fn settled(&self) -> bool {
        let Some(close) = &self.closing else {
            return false;
        };
        if !close.observers_cancelled
            || self.job.is_some()
            || !self.pipe_verified
            || self.pipe_terminal.is_none()
            || self
                .pipe_terminal
                .as_ref()
                .is_some_and(|done| done.scope_id != self.pipe_scope || done.spec != self.pipe_spec)
        {
            return false;
        }
        let work = match self.work.try_lock() {
            Ok(work) => work,
            Err(TryLockError::WouldBlock) => return false,
            Err(TryLockError::Poisoned(poison)) => poison.into_inner(),
        };
        let ui = if let Some(desktop) = &work.desktop {
            desktop.observation_retired()
        } else {
            retired_unbound(&work.cancelled_ui, &work.ui_terminal)
        };
        let handler = if let Some(handler) = &work.handler {
            handler.observation_retired()
        } else {
            retired_unbound(&work.cancelled_handler, &work.handler_terminal)
        };
        ui && handler
    }
}
fn cancel_observers(work: &mut Work) -> Result<(), RegistryError> {
    let mut first = None;
    if let Some(desktop) = &work.desktop {
        desktop.cancel_watch();
    } else if work.cancelled_ui.is_none()
        && let Some(watch) = &mut work.desktop_watch
    {
        match watch.cancel_observation() {
            Ok(job) => work.cancelled_ui = Some(job),
            Err(cause) => first = Some(cause),
        }
    }
    // Continue independent observer retirement even if the first original permit was uncertain.
    if let Some(handler) = &mut work.handler {
        handler.cancel_observation();
    } else if let Some(permit) = work.handler_permit.take() {
        work.cancelled_handler = Some(permit.cancel_unbound());
    }
    first.map_or(Ok(()), Err)
}

fn retired_unbound(
    job: &Option<OwnedJob<ProcessExit, NativeObservationError>>,
    terminal: &Option<JobCompletion<ProcessExit, NativeObservationError>>,
) -> bool {
    job.as_ref()
        .zip(terminal.as_ref())
        .is_some_and(|(job, done)| done.scope_id == job.scope_id() && done.spec == job.spec())
}
