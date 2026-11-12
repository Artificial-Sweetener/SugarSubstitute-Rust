// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Revalidate the original native binding outside the allocation state lock.
use super::*;

impl Owner {
    pub(in crate::diagnostic_storage::allocation) fn native_rejection(
        &self,
    ) -> native::DiagnosticNativeAllocationFailure {
        let operation = self.inner.as_ref().and_then(|inner| inner.lock().current);
        native::DiagnosticNativeAllocationFailure::rejected(operation, Code::Admission)
    }
    pub(in crate::diagnostic_storage::allocation) fn native_metadata(
        &self,
        role: native::NativeRole,
    ) -> Result<Residual, native::DiagnosticNativeAllocationFailure> {
        let layout = role.layout().map_err(|_| {
            native::DiagnosticNativeAllocationFailure::rejected(None, Code::Capacity)
        })?;
        let bytes = layout.retained_bytes();
        if bytes == 0 || bytes > layout.peak_bytes() {
            return Err(native::DiagnosticNativeAllocationFailure::rejected(None, Code::Admission));
        }
        let inner = self.inner.as_ref().ok_or_else(|| {
            native::DiagnosticNativeAllocationFailure::rejected(None, Code::Closed)
        })?;
        let mut state = inner.lock();
        if state.closed {
            return Err(native::DiagnosticNativeAllocationFailure::rejected(
                state.current,
                Code::Closed,
            ));
        }
        let next = state
            .residual
            .checked_add(bytes)
            .filter(|n| *n <= DIAGNOSTIC_RESIDUAL_BYTES)
            .ok_or_else(|| {
                native::DiagnosticNativeAllocationFailure::rejected(state.current, Code::Capacity)
            })?;
        state.residual = next;
        Ok(Residual { owner: self.clone(), bytes })
    }
    pub(in crate::diagnostic_storage::allocation) fn native_current(
        &self,
        role: native::NativeRole,
        current: &JobContext,
    ) -> Result<Residual, native::DiagnosticNativeAllocationFailure> {
        self.native_check(current)?;
        let result = self.native_destination(role);
        let after = self.native_check(current);
        match result {
            Ok(allocation) => {
                after?;
                Ok(allocation)
            }
            Err(cause) => Err(cause.with_post_control(after.err())),
        }
    }
    fn native_destination(
        &self,
        role: native::NativeRole,
    ) -> Result<Residual, native::DiagnosticNativeAllocationFailure> {
        let layout = role.layout().map_err(|_| {
            native::DiagnosticNativeAllocationFailure::rejected(None, Code::Capacity)
        })?;
        let bytes = if role.is_scratch() { layout.peak_bytes() } else { layout.retained_bytes() };
        if bytes == 0 || layout.retained_bytes() > layout.peak_bytes() {
            return Err(native::DiagnosticNativeAllocationFailure::rejected(None, Code::Admission));
        }
        let inner = self.inner.as_ref().ok_or_else(|| {
            native::DiagnosticNativeAllocationFailure::rejected(None, Code::Closed)
        })?;
        let mut state = inner.lock();
        if state.closed || state.current.is_none() || state.native.is_none() {
            return Err(native::DiagnosticNativeAllocationFailure::rejected(
                state.current,
                Code::Closed,
            ));
        }
        let next = state
            .residual
            .checked_add(bytes)
            .filter(|n| *n <= DIAGNOSTIC_RESIDUAL_BYTES)
            .ok_or_else(|| {
                native::DiagnosticNativeAllocationFailure::rejected(state.current, Code::Capacity)
            })?;
        state.residual = next;
        Ok(Residual { owner: self.clone(), bytes })
    }
    pub(in crate::diagnostic_storage::allocation) fn native_check(
        &self,
        current: &JobContext,
    ) -> Result<(), native::DiagnosticNativeAllocationFailure> {
        let inner = self.inner.as_ref().ok_or_else(|| {
            native::DiagnosticNativeAllocationFailure::rejected(None, Code::Closed)
        })?;
        let (snapshot, generation) = {
            let state = inner.lock();
            if state.closed {
                return Err(native::DiagnosticNativeAllocationFailure::rejected(
                    state.current,
                    Code::Closed,
                ));
            }
            (
                state.native.clone().ok_or_else(|| {
                    native::DiagnosticNativeAllocationFailure::rejected(
                        state.current,
                        Code::Admission,
                    )
                })?,
                state.native_generation,
            )
        };
        if !snapshot.matches_context(current) {
            return Err(native::DiagnosticNativeAllocationFailure::rejected(
                Some(snapshot.key()),
                Code::InvalidIdentity,
            ));
        }
        let mut sampling = NativeSampling { owner: self, pending: true };
        let observation = snapshot.observe(current);
        sampling.pending = false;
        let mut state = inner.lock();
        if state
            .namespace
            .as_ref()
            .is_some_and(|source| !source.open.load(std::sync::atomic::Ordering::Acquire))
            || state.capture_source.as_ref().is_some_and(|source| !source.open())
        {
            return Err(native::DiagnosticNativeAllocationFailure::rejected(
                Some(snapshot.key()),
                Code::Closed,
            ));
        }
        if state.closed || state.current != Some(snapshot.key()) {
            return Err(native::DiagnosticNativeAllocationFailure::rejected(
                Some(snapshot.key()),
                Code::Closed,
            ));
        }
        if state.native_generation != generation {
            return Err(native::DiagnosticNativeAllocationFailure::rejected(
                Some(snapshot.key()),
                Code::Busy,
            ));
        }
        let next = match state.native_generation.checked_add(1) {
            Some(next) => next,
            None => {
                state.closed = true;
                return Err(native::DiagnosticNativeAllocationFailure::rejected(
                    Some(snapshot.key()),
                    Code::Capacity,
                ));
            }
        };
        let binding = state.native.as_mut().ok_or_else(|| {
            native::DiagnosticNativeAllocationFailure::rejected(
                Some(snapshot.key()),
                Code::Admission,
            )
        })?;
        let result = binding.validate_observation(observation);
        state.native_generation = next;
        match result {
            Ok(()) => Ok(()),
            Err(failure) => {
                if failure.closes_admission {
                    state.closed = true;
                }
                Err(native::DiagnosticNativeAllocationFailure::rejected(
                    Some(snapshot.key()),
                    failure.code,
                ))
            }
        }
    }
}
struct NativeSampling<'a> {
    owner: &'a Owner,
    pending: bool,
}
impl Drop for NativeSampling<'_> {
    fn drop(&mut self) {
        if self.pending {
            self.owner.close();
        }
    }
}
