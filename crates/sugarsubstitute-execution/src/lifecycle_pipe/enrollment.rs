// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold the single suspended-process handoff until exact acceptance or cancellation.
use super::*;
use crate::task::ErasedJob;

pub(crate) struct Enrollment {
    shared: Arc<Shared>,
    job: Arc<Job<LifecyclePipeReport, LifecyclePipeFailure>>,
    pub(crate) run: RunKey,
    active: bool,
}
impl Enrollment {
    pub(crate) fn matches(&self, shared: &Arc<Shared>) -> bool {
        self.active && Arc::ptr_eq(&self.shared, shared)
    }
    pub(crate) fn take(control: &LifecyclePipeControl) -> Result<Self, LifecyclePipeError> {
        control.commit(|s, _, _| {
            let run = s.run.ok_or(LifecyclePipeError::NotReady)?;
            if s.enrollment_taken || s.enrolled || s.peer.is_some() {
                return Err(LifecyclePipeError::AlreadyTransferred);
            }
            s.enrollment_taken = true;
            s.input_pin = true;
            Ok(Self { shared: control.shared.clone(), job: control.job.clone(), run, active: true })
        })
    }
    pub(crate) fn bind<T>(
        &mut self,
        trusted_run: RunKey,
        key: ProcessKey,
        input: T,
        convert: impl FnOnce(T) -> PeerInput,
    ) -> Result<(), (LifecyclePipeError, T)> {
        let mut input = Some(input);
        let result = control::commit(&self.shared, &self.job, |s, _, _| {
            if !self.active || trusted_run != self.run || key.owner != self.run.owner {
                return Err(LifecyclePipeError::Closed);
            }
            if s.enrolled || s.peer.is_some() {
                return Err(LifecyclePipeError::Busy);
            }
            // Transfer is the final, infallible commit; rejected input remains outside locks.
            if let Some(input) = input.take() {
                s.peer = Some(convert(input));
                s.enrolled = true;
                s.input_pin = false;
                self.active = false;
            }
            Ok(())
        });
        match input {
            Some(input) => Err((result.err().unwrap_or(LifecyclePipeError::Closed), input)),
            None => Ok(()),
        }
    }
}
impl Drop for Enrollment {
    fn drop(&mut self) {
        if self.active {
            {
                lock(&self.shared.state).input_pin = false;
            }
            self.job.cancel();
            self.shared.changed();
        }
    }
}
