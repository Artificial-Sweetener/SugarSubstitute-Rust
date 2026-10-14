// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep issued native custody and its fixed allocation through every synchronous call.
use super::{
    abi,
    control::{ControlCause, ControlHistory, ControlInvocation},
    observation::Failure,
    state::{Original, RUST_DESTINATION_BYTES, State},
};
use std::os::windows::io::AsRawHandle;
use sugarsubstitute_contracts::{
    execution::JobContext,
    incident::{BuildIdentity, DumpId, IncidentKey, IncidentSubject},
};
use sugarsubstitute_execution::{ExecutionClient, windows::NativeObservationError};
use sugarsubstitute_supervision::diagnostic_storage::{
    FINISHED_RUST_STORAGE_BYTES, ProtectedCaptureReadLease, ProtectedFinishedOwnerAllocation,
    ProtectedFinishedRustRetention,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Issue {
    Occupied,
    Identity,
    Current,
    Invalid,
    Native,
    Quarantined,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Unused,
    Issued,
    Retired,
    Quarantined,
}
struct Fields {
    state: State,
    generation: ProtectedCaptureReadLease,
    _context: JobContext,
    phase: Phase,
    counter_ready: bool,
    counter_failure: Option<NativeObservationError>,
    cleanup_counter_ready: bool,
    cleanup_counter_failure: Option<NativeObservationError>,
    source_closed: bool,
    operation: sugarsubstitute_contracts::incident::LogicalStoreOperationKey,
    query: Option<validation::Query>,
    cpp: Option<ProtectedFinishedOwnerAllocation>,
    _rust: ProtectedFinishedRustRetention,
}
/// No public path, token or allocation projection escapes this authoritative adapter.
/// Uncertainty keeps the entire original boxed state and both native/source owners.
pub(crate) struct Token {
    fields: Option<Box<Fields>>,
}
const _: () = assert!(RUST_DESTINATION_BYTES == FINISHED_RUST_STORAGE_BYTES);
const _: () = assert!(
    size_of::<Fields>()
        + super::state::RUST_TEMPORARY_BYTES
        + 2 * size_of::<usize>()
        + super::COPY_CHUNK_BYTES
        + size_of::<crate::incident::collection::CopyState>()
        + size_of::<crate::incident::CapturedReport>()
        + size_of::<sha2::Sha256>()
        + size_of::<crate::incident::CaptureDispositionPlan>()
        + BuildIdentity::MAX_BYTES
        < RUST_DESTINATION_BYTES
);
impl Token {
    /// Install actual output custody before the first clock/provider or native effect.
    /// Rejection leaves every caller-owned Option occupied exactly as supplied.
    pub(in crate::native) fn install(
        output: &mut Option<Self>,
        api: abi::Api,
        generation: &mut Option<ProtectedCaptureReadLease>,
        allocation: &mut Option<ProtectedFinishedOwnerAllocation>,
        rust: &mut Option<ProtectedFinishedRustRetention>,
        context: &JobContext,
    ) -> Result<(), Issue> {
        if output.is_some() {
            return Err(Issue::Occupied);
        }
        let deadline = context.deadline().ok_or(Issue::Current)?;
        if generation.as_ref().zip(allocation.as_ref()).zip(rust.as_ref()).is_none_or(
            |((generation, cpp), rust)| {
                !cpp.matches_generation(generation) || !rust.matches_cpp(cpp)
            },
        ) {
            return Err(Issue::Identity);
        }
        let operation = allocation.as_ref().ok_or(Issue::Identity)?.operation();
        let generation = generation.take().ok_or(Issue::Identity)?;
        let allocation = allocation.take().ok_or(Issue::Identity)?;
        let rust = rust.take().ok_or(Issue::Identity)?;
        *output = Some(Self {
            fields: Some(Box::new(Fields {
                state: State {
                    api,
                    token: 0,
                    unexpected: None,
                    history: ControlHistory::default(),
                    original: Original {
                        execution: context.owner_id(),
                        scope: context.scope_id(),
                        job: context.spec().id,
                        deadline,
                        counter: abi::Deadline::default(),
                    },
                    cleanup: None,
                    facts: None,
                    preparation: None,
                    failures: super::observation::Failures::new(),
                },
                generation,
                _context: context.clone(),
                phase: Phase::Unused,
                counter_ready: false,
                counter_failure: None,
                cleanup_counter_ready: false,
                cleanup_counter_failure: None,
                source_closed: false,
                operation,
                query: None,
                cpp: Some(allocation),
                _rust: rust,
            })),
        });
        Ok(())
    }
    /// Translate through the original execution owner's genuine QPC adapter once.
    /// The trusted composing Capture source supplies its retained SAME client.
    pub(crate) fn encode_deadline(&mut self, client: &ExecutionClient) -> Result<(), Issue> {
        let fields = self.fields.as_mut().ok_or(Issue::Identity)?;
        if fields.phase != Phase::Unused || fields.counter_ready || fields.counter_failure.is_some()
        {
            return Err(Issue::Occupied);
        }
        match client.encode_counter_deadline(fields.state.original.deadline) {
            Ok(counter) => {
                fields.state.original.counter =
                    abi::Deadline { counter: counter.ticks(), frequency: counter.frequency() };
                fields.counter_ready = true;
                Ok(())
            }
            Err(cause) => {
                fields.counter_failure = Some(cause);
                Err(Issue::Current)
            }
        }
    }
}
impl Fields {
    fn check(&self, context: &JobContext) -> Result<(), Issue> {
        if self.phase == Phase::Quarantined {
            return Err(Issue::Quarantined);
        }
        let original = &self.state.original;
        if (original.execution, original.scope, original.job)
            != (context.owner_id(), context.scope_id(), context.spec().id)
            || context.deadline().is_none_or(|until| until > original.deadline)
        {
            return Err(Issue::Identity);
        }
        Ok(())
    }
    fn observe(
        &mut self,
        output: &abi::Output,
        control: Option<super::control::ControlCause>,
    ) -> Result<(), Issue> {
        let previous = self.state.facts;
        if self.state.observe(output, control).is_err() {
            self.phase = Phase::Quarantined;
            return Err(Issue::Invalid);
        }
        if output.facts_valid == 1
            && (self.query.as_ref().is_none_or(|query| !query.matches(&output.facts))
                || previous.as_ref().is_some_and(|old| {
                    old.uuid != output.facts.uuid
                        || old.source_bytes != output.facts.source_bytes
                        || old.source_identity != output.facts.source_identity
                }))
        {
            self.phase = Phase::Quarantined;
            self.state.failures.retain(Failure::Invalid(super::observation::InvalidResult));
            return Err(Issue::Invalid);
        }
        Ok(())
    }
    fn accept_retirement(&mut self, output: &abi::Output) {
        if output.retired == 1 {
            self.phase = Phase::Retired;
            self.state.token = 0;
            drop(self.cpp.take());
        }
    }
}
impl Drop for Token {
    fn drop(&mut self) {
        if let Some(fields) = self.fields.take() {
            if fields.source_closed && matches!(fields.phase, Phase::Unused | Phase::Retired) {
                drop(fields);
            } else {
                // Preserve actual token, generation, original causes and both guard roles.
                let _retained = Box::leak(fields);
            }
        }
    }
}

#[path = "token/calls.rs"]
mod calls;
#[path = "token/disposition.rs"]
mod disposition;
#[path = "token/facts.rs"]
mod facts;
#[path = "token/plan.rs"]
mod plan;
#[path = "token/preparation.rs"]
mod preparation;
#[path = "token/retirement.rs"]
mod retirement;
#[cfg(test)]
#[path = "token/tests.rs"]
mod tests;
#[path = "token/validation.rs"]
mod validation;
