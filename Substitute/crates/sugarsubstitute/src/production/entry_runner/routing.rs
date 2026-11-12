// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route one immutable completion batch and real authenticated loans to their retained owners.
use super::super::entry::EntrySide;
use super::*;
use sugarsubstitute_contracts::{
    coordinator_entry::EntryOutcome,
    execution::{JobClass, JobSpec, Operation},
};
use sugarsubstitute_execution::{EntryTransportObservation, MIN_ENTRY_CLIENT_BYTES};
use sugarsubstitute_supervision::coordinator_entry::EntryPhase;

impl<C: EntryActivation> EntryRunner<C> {
    pub(super) fn route(&mut self, batch: &CompletionBatch) -> Result<(), Box<dyn Error>> {
        if self.post_exit.is_some() {
            return self.advance_updates(batch);
        }
        let mut activation_attempted = false;
        let client = self.client()?.clone();
        let bootstrap_error = self
            .bootstrap
            .as_mut()
            .and_then(|bootstrap| bootstrap.consume(batch, &self.jobs).err());
        if let Some(cause) = bootstrap_error {
            self.first_origin.get_or_insert(FailureOrigin::Bootstrap);
            self.close_on_failure(Box::new(cause));
        }
        let mut authority_error = None;
        if let (Some(bootstrap), Some(authority), Some(finite)) =
            (&mut self.bootstrap, &mut self.authority, &self.finite)
            && let Some(entry) = bootstrap.owner_mut()
        {
            if !self.closing
                && !self.authority_started
                && entry.snapshot().phase == EntryPhase::ElectionHeld
            {
                self.authority_started = true;
                if let Err(cause) =
                    authority.prepare(entry, &client, finite, &self.jobs, self.installation.clone())
                {
                    authority_error = Some(cause);
                }
            }
            if self.authority_started
                && let Err(cause) = authority.consume(entry, &client, finite, &self.jobs, batch)
            {
                authority_error = Some(cause);
            }
            if !self.closing && authority.serving() && self.intake.is_none() {
                self.intake = Some(ProductionEntryIntake::new(
                    client.clone(),
                    client.create_native_scope(
                        ScopeSpec { id: self.scopes.allocate()?, deadline: None },
                        None,
                    )?,
                    self.jobs.clone(),
                    entry,
                )?);
            }
            if !self.closing
                && self.forward.is_none()
                && matches!(entry.snapshot().phase, EntryPhase::Candidate | EntryPhase::Contended)
            {
                let input = entry.client_input()?.with_unsubmitted_recovery();
                let spec = JobSpec {
                    id: self.jobs.allocate()?,
                    operation: Operation::NativeLifecycle,
                    class: JobClass::NativeObservation,
                    deadline: None,
                    reservation_bytes: MIN_ENTRY_CLIENT_BYTES,
                    replacement: None,
                };
                self.forward = Some(client.reserve_coordinator_entry_client(
                    self.native.as_ref().ok_or("Production native scope missing")?,
                    spec,
                    input,
                )?);
            }
        }
        if let Some(cause) = authority_error {
            self.first_origin.get_or_insert(FailureOrigin::Authority);
            self.close_on_failure(Box::new(cause));
        }
        self.poll_intake(&mut activation_attempted)?;
        if let Some(forward) = &self.forward {
            let mut outcome = None;
            for _ in 0..32 {
                let Some(observed) = forward.control.take_observation() else {
                    break;
                };
                tracing::info!(
                    operation = "entry_forward_progress",
                    ?observed,
                    "Existing coordinator request advanced"
                );
                if let EntryTransportObservation::OutcomeReceived { outcome: actual, .. } = observed
                {
                    outcome = Some(actual);
                }
            }
            forward.control.rearm_observations();
            if let Some(outcome) = outcome {
                match outcome {
                    EntryOutcome::Accepted(_) => {
                        self.initial_completed = true;
                        self.request_shutdown()?;
                    }
                    EntryOutcome::Rejected(reason) => {
                        self.retain(Box::new(Rejected(reason)));
                        return Err("Existing coordinator rejected the entry request".into());
                    }
                }
            }
        }
        self.observe_transport_completions(&mut activation_attempted)?;
        self.advance_updates(batch)?;
        if self.closing {
            if let Some(budget) = self.shutdown {
                let (child, resources) = self.parts_optional()?;
                let closed = child.poll_close(batch, resources, budget);
                match closed {
                    Ok(actual) => self.child_closed = actual,
                    Err(cause) => {
                        self.first_origin.get_or_insert(FailureOrigin::Child);
                        self.retain(cause);
                    }
                }
            }
            self.observe_retained_child_failure();
            if self.failure.is_some() && !self.failure_reported {
                self.failure_reported = true;
                return Err("Production ownership failure remains retained".into());
            }
            return Ok(());
        }
        if self.pending_activation.is_some() && !activation_attempted {
            self.enter_pending()?;
        }
        if self.intake_ready {
            let (child, resources) = self.parts()?;
            let outcome = child.consume(batch, resources);
            let step = match outcome {
                Ok(step) => step,
                Err(cause) => {
                    self.first_origin.get_or_insert(FailureOrigin::Child);
                    self.retain(cause);
                    return Err("Actual child owner failed with retained original cause".into());
                }
            };
            if step == HeadlessStep::Shutdown {
                self.request_shutdown()?;
            }
        }
        if self.observe_retained_child_failure() {
            return Err("Actual child owner failed with retained original cause".into());
        }
        if !self.initial_completed
            && let Some(original) = self.original
            && self.client()?.now()? >= original.until()
        {
            tracing::error!(operation = "entry_setup_expired",
                bootstrap = ?self.bootstrap.as_ref().and_then(EntryBootstrap::owner).map(|owner| owner.snapshot()),
                forwarding = self.forward.is_some(), authority_started = self.authority_started,
                intake_ready = self.intake_ready,
                "Entry setup reached its deadline before admission");
            return Err(format!(
                "Production entry setup expired before actual admission: phase={:?}, authority={}, intake={}, forwarding={}",
                self.bootstrap.as_ref().and_then(EntryBootstrap::owner).map(|owner| owner.snapshot().phase),
                self.authority_started, self.intake_ready, self.forward.is_some()
            ).into());
        }
        self.try_normal_shutdown()?;
        Ok(())
    }
    /// Route one bounded intake batch through the same current entry owner.
    fn poll_intake(&mut self, activation_attempted: &mut bool) -> Result<(), Box<dyn Error>> {
        if let (Some(intake), Some(bootstrap)) = (&mut self.intake, &mut self.bootstrap)
            && let Some(entry) = bootstrap.owner_mut()
        {
            let events = intake.poll(entry).into_events();
            for event in events {
                if let Some(failure) = event.failure {
                    self.retain(Box::new(failure));
                    return Err("Actual entry intake rejected an observation".into());
                }
                match (event.side, event.observation) {
                    (EntrySide::Server, EntryTransportObservation::ServerActivated { .. }) => {
                        self.intake_ready = true
                    }
                    (
                        EntrySide::InitialClient,
                        EntryTransportObservation::OutcomeReceived { outcome, .. },
                    ) if !self.closing => {
                        let authority = self
                            .authority
                            .as_ref()
                            .filter(|owner| owner.serving())
                            .and_then(CoordinatorAuthority::key);
                        let entry = self
                            .bootstrap
                            .as_ref()
                            .and_then(EntryBootstrap::owner)
                            .filter(|owner| owner.snapshot().phase == EntryPhase::Serving)
                            .and_then(|owner| owner.snapshot().owner);
                        self.initial_completed = initial_outcome(outcome, authority, entry)?;
                    }
                    (
                        EntrySide::Server,
                        EntryTransportObservation::RequestAuthenticated { request, .. },
                    ) if !self.closing => {
                        if self.pending_activation.is_some() {
                            return Err(
                                "Original activation still awaits actual child admission".into()
                            );
                        }
                        self.pending_activation = Some(request);
                        *activation_attempted = true;
                        self.enter_pending()?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
    fn enter_pending(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(request) = self.pending_activation.as_ref() else {
            return Ok(());
        };
        let key = self
            .authority
            .as_ref()
            .and_then(CoordinatorAuthority::key)
            .ok_or("Actual coordinator authority missing")?;
        let entry = self
            .bootstrap
            .as_ref()
            .and_then(EntryBootstrap::owner)
            .ok_or("Actual entry owner missing")?;
        if !self.authority.as_ref().is_some_and(CoordinatorAuthority::serving) {
            return Err("Coordinator authority is not serving".into());
        }
        entry.validate_current_owner(key, request.peer())?;
        request.view()?;
        let now = self.client()?.now()?;
        if now < request.received_at() || now >= request.until() {
            return Err("Original authenticated request expired".into());
        }
        let mut pending = self.pending_activation.take();
        let result = match self.parts() {
            Ok((child, resources)) => child.enter_activation(&mut pending, resources),
            Err(cause) => Err(cause),
        };
        self.pending_activation = pending;
        if let Err(cause) = result {
            self.first_origin.get_or_insert(FailureOrigin::Child);
            self.retain(cause);
            return Err("Actual child activation failed".into());
        }
        Ok(())
    }
    fn parts(&mut self) -> Result<(&mut C, EntryResources<'_>), Box<dyn Error>> {
        let entry = self
            .bootstrap
            .as_mut()
            .and_then(EntryBootstrap::owner_mut)
            .ok_or("Actual entry owner missing")?;
        let authority = self.authority.as_mut().ok_or("Actual authority missing")?;
        let intake = self.intake.as_mut().ok_or("Actual intake missing")?;
        Ok((
            self.child.as_mut(),
            EntryResources {
                client: self.client.as_ref().ok_or("Execution client missing")?,
                finite: self.finite.as_ref().ok_or("Finite scope missing")?,
                native: self.native.as_ref().ok_or("Native scope missing")?,
                cleanup: self.cleanup.as_ref().ok_or("Cleanup scope missing")?,
                jobs: &self.jobs,
                scopes: &self.scopes,
                entry,
                authority,
                intake,
            },
        ))
    }
    fn parts_optional(&mut self) -> Result<(&mut C, Option<EntryResources<'_>>), Box<dyn Error>> {
        if self.intake.is_none() {
            return Ok((self.child.as_mut(), None));
        }
        self.parts().map(|(child, resources)| (child, Some(resources)))
    }
    fn observe_transport_completions(
        &mut self,
        activation_attempted: &mut bool,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(intake) = &mut self.intake {
            if self.transport_completions[0].is_none() {
                self.transport_completions[0] = intake.server().completion.try_take_completion();
            }
            if self.transport_completions[1].is_none()
                && let Some(client) = intake.initial_client()
            {
                self.transport_completions[1] = client.completion.try_take_completion();
            }
        }
        if self.transport_completions[2].is_none()
            && let Some(forward) = &mut self.forward
        {
            self.transport_completions[2] = forward.completion.try_take_completion();
        }
        self.recover_unsubmitted_forward()?;
        let initial = self.transport_completions[1]
            .as_ref()
            .filter(|_| {
                !self.closing
                    && self.transport_completions[0].is_none()
                    && self.transport_completions[2].is_none()
            })
            .map(|completion| completion.outcome.kind());
        self.initial_completed = reconcile_initial(initial, self.initial_completed, || {
            // Actual terminal publication follows outcome emission. This second
            // fair32 batch drains all <=8 now-quiescent initial-client facts.
            self.poll_intake(activation_attempted)?;
            Ok(self.initial_completed)
        })?;
        let unexpected = (0..3).find(|index| {
            premature_transport(
                *index,
                self.transport_completions[*index]
                    .as_ref()
                    .map(|completion| completion.outcome.kind()),
                self.initial_completed,
            )
        });
        if !self.closing
            && let Some(index) = unexpected
        {
            self.first_origin.get_or_insert(FailureOrigin::Transport(index));
            if let Some(completion) = &self.transport_completions[index] {
                use sugarsubstitute_contracts::execution::JobOutcome;
                let role = ["server", "initial_client", "forwarded_client"][index];
                match &completion.outcome {
                    JobOutcome::Success(report) => tracing::error!(
                        operation = "production_transport_terminated",
                        role,
                        status = "success",
                        retired_connections = report.retired_connections,
                        authenticated_launches = report.authenticated_launches,
                        completed_writes = report.completed_writes,
                        "Entry transport retired before its application outcome was adopted"
                    ),
                    JobOutcome::Failure(cause) => tracing::error!(
                        operation="production_transport_terminated",role,status="failure",
                        failure=?FailureDisplay(cause),
                        "Entry transport failed with its original retained terminal cause"
                    ),
                    JobOutcome::Cancelled => tracing::error!(
                        operation = "production_transport_terminated",
                        role,
                        status = "cancelled",
                        "Entry transport cancelled before admission closed"
                    ),
                    JobOutcome::TimedOut => tracing::error!(
                        operation = "production_transport_terminated",
                        role,
                        status = "deadline",
                        "Entry transport exceeded its deadline before admission closed"
                    ),
                    JobOutcome::Panicked(_) => tracing::error!(
                        operation = "production_transport_terminated",
                        role,
                        status = "panicked",
                        "Entry transport panicked before admission closed"
                    ),
                    JobOutcome::Coalesced { .. } => tracing::error!(
                        operation = "production_transport_terminated",
                        role,
                        status = "coalesced",
                        "Entry transport coalesced before admission closed"
                    ),
                }
            }
            return Err("Actual entry transport returned before admission closed".into());
        }
        let client_exists =
            self.intake.as_mut().is_some_and(|intake| intake.initial_client().is_some());
        self.intake_settled = (self.intake.is_none()
            || self.transport_completions[0].is_some()
                && (!client_exists || self.transport_completions[1].is_some()))
            && (self.forward.is_none() || self.transport_completions[2].is_some());
        Ok(())
    }
}
#[derive(Debug)]
struct Rejected(sugarsubstitute_contracts::coordinator_entry::EntryRejection);
impl fmt::Display for Rejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Authenticated entry request rejected: {:?}", self.0)
    }
}
impl Error for Rejected {}

/// Reconcile only a successful initial terminal with its independently queued facts.
fn reconcile_initial(
    terminal: Option<sugarsubstitute_contracts::execution::TerminalKind>,
    completed: bool,
    poll: impl FnOnce() -> Result<bool, Box<dyn Error>>,
) -> Result<bool, Box<dyn Error>> {
    if terminal == Some(sugarsubstitute_contracts::execution::TerminalKind::Success) && !completed {
        poll()
    } else {
        Ok(completed)
    }
}
/// Server and forward termination remain fatal; an initial failure never hides behind Accepted.
fn premature_transport(
    index: usize,
    terminal: Option<sugarsubstitute_contracts::execution::TerminalKind>,
    completed: bool,
) -> bool {
    terminal.is_some_and(|kind| {
        index != 1
            || !completed
            || kind != sugarsubstitute_contracts::execution::TerminalKind::Success
    })
}
/// Sealed transport observations retain exact request/source/nonce correlation.
/// Adoption additionally requires the complete returned Run's actual current owner.
fn initial_outcome(
    outcome: EntryOutcome,
    authority: Option<sugarsubstitute_contracts::lifecycle::CoordinatorKey>,
    entry: Option<sugarsubstitute_contracts::lifecycle::CoordinatorKey>,
) -> Result<bool, Box<dyn Error>> {
    match outcome {
        EntryOutcome::Accepted(run) if authority == Some(run.owner) && entry == authority => {
            Ok(true)
        }
        EntryOutcome::Accepted(_) => Err(Box::new(EntryTransportError::Authentication)),
        EntryOutcome::Rejected(reason) => Err(Box::new(Rejected(reason))),
    }
}

#[cfg(test)]
#[path = "initial_terminal_tests.rs"]
mod initial_terminal_tests;
