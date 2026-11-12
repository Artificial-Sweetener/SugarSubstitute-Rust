// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold the single suspended-process handoff until exact acceptance or cancellation.
use super::*;

pub(crate) struct Enrollment {
    owner: Owner,
    shared: SharedRef,
    active: bool,
}
impl Enrollment {
    pub(crate) fn binding(&self) -> Option<PeerBinding> {
        lock(&self.shared.state).binding
    }
    pub(crate) fn matches(&self, shared: &SharedRef) -> bool {
        self.active && self.shared.same_allocation(shared)
    }
    pub(crate) fn take(control: &Control) -> Result<Self, LifecyclePipeError> {
        control.commit(|s, _, _| {
            s.binding.ok_or(LifecyclePipeError::NotReady)?;
            if s.enrollment_taken || s.enrolled || s.peer.is_some() {
                return Err(LifecyclePipeError::AlreadyTransferred);
            }
            s.enrollment_taken = true;
            s.input_pin = true;
            Ok(Self { shared: control.shared.clone(), owner: control.owner.clone(), active: true })
        })
    }
    pub(crate) fn take_bootstrap(
        control: &Control,
    ) -> Result<Option<(LifecyclePipeBootstrap, Self)>, LifecyclePipeError> {
        control.commit(|s, _, _| {
            s.binding.ok_or(LifecyclePipeError::NotReady)?;
            if s.transferred || s.enrollment_taken || s.enrolled || s.peer.is_some() {
                return Err(LifecyclePipeError::AlreadyTransferred);
            }
            if !s.prepared || s.inherited == 0 {
                return Ok(None);
            }
            s.transferred = true;
            s.pin = true;
            s.enrollment_taken = true;
            s.input_pin = true;
            Ok(Some((
                LifecyclePipeBootstrap { shared: control.shared.clone() },
                Self { shared: control.shared.clone(), owner: control.owner.clone(), active: true },
            )))
        })
    }
    pub(crate) fn bind<T>(
        &mut self,
        trusted: PeerBinding,
        key: ProcessKey,
        input: T,
        convert: impl FnOnce(T) -> PeerInput,
    ) -> Result<(), (LifecyclePipeError, T)> {
        let mut input = Some(input);
        let result = commands::commit(&self.shared, &self.owner, |s, _, _| {
            if !self.active || Some(trusted) != s.binding || key.owner != trusted.owner() {
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
            self.owner.cancel();
            self.shared.changed();
        }
    }
}
