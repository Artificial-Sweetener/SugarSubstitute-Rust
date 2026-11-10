// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain compositor waits in the application's existing execution lifetime.

use crate::job_sequence::JobSequence;
use std::{error::Error, io};
use sugarsubstitute_contracts::{
    execution::{
        AdmissionError, ExecutionPort, JobClass, JobOutcome, JobSpec, Moment, Operation, OwnedJob,
        Scope, ScopeSpec,
    },
    startup::MainIdentity,
};
use sugarsubstitute_desktop::canvas::{CompositionFence, CompositionFenceError, CompositionWait};
use sugarsubstitute_execution::{ExecutionClient, OwnedInputError};

/// A wait ends when the compositor completes it; only the shutdown cutoff, once a close began,
/// bounds it, so a compositor that is slow under load delays the wait instead of failing it.
struct Pending {
    input: Option<CompositionWait>,
    fence: CompositionFence,
    deadline: Option<Moment>,
}

struct Active {
    fence: CompositionFence,
    job: OwnedJob<(), CompositionFenceError>,
    deadline: Option<Moment>,
    expiry_reported: bool,
}

/// One bounded wait at a time, retained through the actual main-window lifetime.
pub(super) struct CompositionWork {
    spec: ScopeSpec,
    jobs: JobSequence,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    main: Option<MainIdentity>,
    native_main_retired: bool,
    pending: Option<Pending>,
    active: Option<Active>,
    closing: bool,
    cancelled: bool,
    finished: bool,
}

impl CompositionWork {
    /// Keep the scope inert until the existing application executor starts.
    pub(super) fn new(spec: ScopeSpec, jobs: JobSequence) -> Self {
        Self {
            spec,
            jobs,
            client: None,
            scope: None,
            main: None,
            native_main_retired: false,
            pending: None,
            active: None,
            closing: false,
            cancelled: false,
            finished: false,
        }
    }

    /// Preserve monotonic scope creation when the canvas starts later.
    pub(super) fn scope_id(&self) -> sugarsubstitute_contracts::execution::ScopeId {
        self.spec.id
    }

    /// Create this independent scope after the earlier startup scopes have been admitted.
    pub(super) fn start(&mut self, client: ExecutionClient) -> Result<(), Box<dyn Error>> {
        if self.client.is_some() || self.closing {
            return Err("Composition execution cannot restart".into());
        }
        let scope = client.create_scope(self.spec, None)?;
        self.client = Some(client);
        self.scope = Some(scope);
        Ok(())
    }

    /// Retain the exact native main before it can create a composition tree.
    pub(super) fn register(&mut self, identity: MainIdentity) -> Result<bool, Box<dyn Error>> {
        if self.main == Some(identity) {
            return Ok(true);
        }
        if self.native_main_retired && self.main.is_some() {
            return Ok(false);
        }
        if self.closing || self.main.is_some() || self.scope.is_none() {
            return Err("Composition main lifetime is unavailable".into());
        }
        self.main = Some(identity);
        Ok(true)
    }

    /// Accept one original native wait, leaving it with the desktop while capacity is busy.
    pub(super) fn offer(
        &mut self,
        identity: MainIdentity,
        input: &mut Option<CompositionWait>,
        now: Moment,
        cutoff: Option<Moment>,
    ) -> Result<bool, Box<dyn Error>> {
        if self.main != Some(identity) || self.cancelled {
            return Err("Composition wait belongs to an unavailable main".into());
        }
        if self.pending.is_some() || self.active.is_some() {
            return Ok(false);
        }
        let input = input.take().ok_or("Composition wait input absent")?;
        let fence = input.fence();
        self.pending = Some(Pending { input: Some(input), fence, deadline: cutoff });
        self.advance(now)?;
        Ok(true)
    }

    /// Observe the worker's real terminal result and retry retained admission when feasible.
    pub(super) fn advance(&mut self, now: Moment) -> Result<(), Box<dyn Error>> {
        if let Some(active) = &mut self.active
            && let Some(completion) = active.job.try_take_completion()
        {
            let active = self.active.take().ok_or("Composition wait disappeared")?;
            match completion.outcome {
                JobOutcome::Success(()) if active.fence.completed() => {}
                JobOutcome::Cancelled | JobOutcome::TimedOut if active.fence.completed() => {}
                JobOutcome::Failure(cause) => {
                    active.fence.terminal_without_completion();
                    return Err(Box::new(cause));
                }
                outcome => {
                    active.fence.terminal_without_completion();
                    return Err(io::Error::other(format!(
                        "Composition wait ended: {:?}",
                        outcome.kind()
                    ))
                    .into());
                }
            }
            self.release_retired_main();
        }
        if let Some(active) = &mut self.active
            && !active.expiry_reported
            && !active.fence.completed()
            && active.deadline.is_some_and(|deadline| now >= deadline)
        {
            active.expiry_reported = true;
            return Err(io::Error::from(io::ErrorKind::TimedOut).into());
        }
        let Some(pending) = &mut self.pending else { return Ok(()) };
        if pending.deadline.is_some_and(|deadline| now >= deadline) {
            pending.fence.terminal_without_completion();
            return Err(io::Error::from(io::ErrorKind::TimedOut).into());
        }
        let client = self.client.as_ref().ok_or("Composition execution absent")?;
        let scope = self.scope.as_ref().ok_or("Composition scope absent")?;
        let id = match self.jobs.allocate() {
            Ok(id) => id,
            Err(cause) => {
                pending.fence.terminal_without_completion();
                return Err(cause.into());
            }
        };
        let spec = JobSpec {
            id,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: pending.deadline,
            reservation_bytes: 64 * 1024,
            replacement: None,
        };
        match client.submit_blocking_owned_input(
            scope,
            spec,
            &mut pending.input,
            |input, context| input.wait(&context),
        ) {
            Ok(job) => {
                let pending = self.pending.take().ok_or("Composition wait disappeared")?;
                self.active = Some(Active {
                    fence: pending.fence,
                    job,
                    deadline: pending.deadline,
                    expiry_reported: false,
                });
            }
            Err(OwnedInputError::Admission(
                AdmissionError::QueueSaturated { .. }
                | AdmissionError::MemorySaturated { .. }
                | AdmissionError::CompletionSaturated,
            )) => {}
            Err(cause) => {
                pending.fence.terminal_without_completion();
                return Err(cause.into());
            }
        }
        Ok(())
    }

    /// Return the retained admission cutoff without inventing a periodic timer.
    pub(super) fn deadline(&self) -> Option<Moment> {
        self.pending.as_ref().and_then(|pending| pending.deadline).or_else(|| {
            self.active
                .as_ref()
                .filter(|active| !active.expiry_reported)
                .and_then(|active| active.deadline)
        })
    }

    /// Accept native retirement only for the registered main and completed fences.
    pub(super) fn main_retired(&mut self, identity: MainIdentity) -> Result<(), Box<dyn Error>> {
        if self.main.is_none() {
            return Ok(());
        }
        if self.main != Some(identity) {
            return Err("Composition retirement identity changed".into());
        }
        if self.pending.as_ref().is_some_and(|pending| !pending.fence.completed())
            || self.active.as_ref().is_some_and(|active| !active.fence.completed())
        {
            return Err("Composition retired before its wait completed".into());
        }
        self.native_main_retired = true;
        self.release_retired_main();
        Ok(())
    }

    /// Keep a retired identity installed until its execution owner publishes terminal completion.
    fn release_retired_main(&mut self) {
        if self.native_main_retired && self.pending.is_none() && self.active.is_none() {
            self.main = None;
            self.native_main_retired = false;
        }
    }

    /// Close new main admission while preserving outstanding native wait custody.
    pub(super) fn close(&mut self) {
        self.closing = true;
    }

    /// Preserve the original shutdown cutoff for a wait still awaiting admission.
    pub(super) fn tighten(&mut self, cutoff: Moment) {
        if let Some(pending) = &mut self.pending {
            pending.deadline =
                Some(pending.deadline.map_or(cutoff, |deadline| deadline.min(cutoff)));
        }
        if let Some(active) = &mut self.active {
            active.deadline = Some(active.deadline.map_or(cutoff, |deadline| deadline.min(cutoff)));
        }
    }

    /// Cancel the scope only after the main and all actual wait jobs have retired.
    pub(super) fn settled(&mut self, now: Moment) -> Result<bool, Box<dyn Error>> {
        self.advance(now)?;
        if !self.closing || self.main.is_some() || self.pending.is_some() || self.active.is_some() {
            return Ok(false);
        }
        let Some(scope) = &self.scope else { return Ok(true) };
        if !self.cancelled {
            scope.cancel();
            self.cancelled = true;
        }
        let snapshot = scope.snapshot();
        Ok(snapshot.cancellation_requested
            && snapshot.queued == 0
            && snapshot.running == 0
            && snapshot.terminal_unobserved == 0
            && snapshot.children_live == 0
            && snapshot.reserved_bytes == 0
            && snapshot.native_reservations == 0
            && snapshot.native_outputs_in_custody == 0)
    }

    /// Release the already-settled scope after native and worker retirement.
    pub(super) fn finish(&mut self, now: Moment) -> Result<(), Box<dyn Error>> {
        if !self.settled(now)? {
            return Err("Composition execution remains unsettled".into());
        }
        self.scope = None;
        self.client = None;
        self.finished = true;
        Ok(())
    }
}

impl Drop for CompositionWork {
    fn drop(&mut self) {
        if !self.finished {
            std::mem::forget(self.pending.take());
            std::mem::forget(self.active.take());
            std::mem::forget(self.scope.take());
        }
    }
}

#[cfg(test)]
mod tests;
