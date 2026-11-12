// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep checked native retirement separate from generation closure and Rust deallocation.
use super::*;
impl Token {
    /// Checked-close only the genuine known token on the separately admitted cleanup context.
    /// A foreign or malformed return quarantines every further effect, including cleanup.
    pub(crate) fn retire_cpp(
        &mut self,
        client: &ExecutionClient,
        context: &JobContext,
    ) -> Result<bool, Issue> {
        let fields = self.fields.as_mut().ok_or(Issue::Identity)?;
        if fields.phase == Phase::Quarantined {
            return Err(Issue::Quarantined);
        }
        let deadline = context.deadline().ok_or(Issue::Current)?;
        let now = context.now();
        if deadline <= now || deadline.as_nanos() - now.as_nanos() > 15_000_000_000 {
            return Err(Issue::Current);
        }
        if fields.state.original.execution != context.owner_id()
            || fields
                .state
                .cleanup
                .as_ref()
                .is_some_and(|old| old.scope != context.scope_id() || deadline > old.deadline)
        {
            return Err(Issue::Identity);
        }
        if matches!(fields.phase, Phase::Unused | Phase::Retired) {
            return Ok(true);
        }
        if fields.phase != Phase::Issued || fields.state.token == 0 {
            return Err(Issue::Current);
        }
        if fields.state.cleanup.is_none() {
            fields.state.cleanup = Some(Original {
                execution: context.owner_id(),
                scope: context.scope_id(),
                job: context.spec().id,
                deadline,
                counter: abi::Deadline::default(),
            });
        }
        if !fields.cleanup_counter_ready {
            let original_deadline = fields.state.cleanup.as_ref().ok_or(Issue::Identity)?.deadline;
            let encoded = client.encode_counter_deadline(original_deadline).map_err(|cause| {
                fields.cleanup_counter_failure.get_or_insert(cause);
                Issue::Current
            })?;
            fields.state.cleanup.as_mut().ok_or(Issue::Identity)?.counter =
                abi::Deadline { counter: encoded.ticks(), frequency: encoded.frequency() };
            fields.cleanup_counter_ready = true;
        }
        let counter = fields.state.cleanup.as_ref().ok_or(Issue::Identity)?.counter;
        let mut current = ControlInvocation::new(context, &mut fields.state.history);
        let call = abi::Call {
            version: 1,
            phase: 2,
            lease: fields.state.token,
            original: counter,
            current: current.source(),
        };
        let mut output = abi::Output::empty();
        // SAFETY: known opaque token, same preheld role, and synchronous borrowed cleanup source.
        unsafe { (fields.state.api.retire)(&call, &mut output) };
        let control = current.take_failure();
        drop(current);
        fields.observe(&output, control)?;
        fields.accept_retirement(&output);
        Ok(fields.phase == Phase::Retired)
    }
    /// Close the genuine generation after known-unused or checked-retired CPP custody.
    /// The distinct Rust guard funds all state and causes until their final deallocation.
    pub(crate) fn retire_source(&mut self, context: &JobContext) -> Result<(), Issue> {
        let fields = self.fields.as_mut().ok_or(Issue::Identity)?;
        if !matches!(fields.phase, Phase::Unused | Phase::Retired) {
            return Err(Issue::Quarantined);
        }
        if fields.phase == Phase::Unused {
            drop(fields.cpp.take());
        }
        if fields.source_closed {
            return Ok(());
        }
        fields.generation.retire(context).map_err(|cause| {
            fields.state.failures.retain(Failure::Control(ControlCause::Source(cause)));
            Issue::Current
        })?;
        fields.source_closed = true;
        Ok(())
    }
    /// Borrow the original actual source/native control causes without detached allocation.
    pub(super) fn failure(&self) -> Option<&Failure> {
        self.fields.as_ref().and_then(|fields| fields.state.failures.first.as_ref())
    }
    /// Borrow a genuine original QPC/central-clock translation error.
    pub(crate) fn counter_failure(&self) -> Option<&NativeObservationError> {
        self.fields.as_ref().and_then(|fields| fields.counter_failure.as_ref())
    }
}
