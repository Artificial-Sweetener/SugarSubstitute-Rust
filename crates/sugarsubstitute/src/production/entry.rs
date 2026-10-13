// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual entry sessions and route their original authenticated observations.
use crate::job_sequence::JobSequence;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::{JobClass, JobSpec, Operation, Scope},
};
use sugarsubstitute_execution::{
    CoordinatorEntryClientSession, CoordinatorEntryServerSession, EntryTransportAdmissionError,
    EntryTransportError, EntryTransportObservation, ExecutionClient, MIN_ENTRY_CLIENT_BYTES,
    MIN_ENTRY_SERVER_BYTES,
};
use sugarsubstitute_supervision::coordinator_entry::{
    CoordinatorEntryOwner, EntryAdmissionError, EntryPhase,
};

/// Distinguish the actual receiving endpoint without classifying child readiness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntrySide {
    Server,
    InitialClient,
}
/// Preserve the original rejected operation cause.
#[derive(Debug)]
pub(crate) enum EntryIntakeFailure {
    Owner(EntryAdmissionError),
    Admission(EntryTransportAdmissionError),
    Transport(EntryTransportError),
    Sequence(sugarsubstitute_contracts::execution::IdentityError),
    Clock(sugarsubstitute_contracts::execution::ExecutionError),
    Identity,
}
impl fmt::Display for EntryIntakeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Production entry intake failed")
    }
}
impl Error for EntryIntakeFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Owner(cause) => Some(cause),
            Self::Admission(cause) => Some(cause),
            Self::Transport(cause) => Some(cause),
            Self::Identity => None,
            Self::Sequence(cause) => Some(cause),
            Self::Clock(cause) => Some(cause),
        }
    }
}
/// Move every original fact/frame and any rejection cause to the composition owner.
pub(crate) struct EntryIntakeEvent {
    pub(crate) side: EntrySide,
    pub(crate) observation: EntryTransportObservation,
    pub(crate) failure: Option<EntryIntakeFailure>,
}
/// Keep one bounded stack batch; frame loans retain actual native custody separately.
pub(crate) struct EntryIntakeBatch {
    events: [Option<EntryIntakeEvent>; 32],
}
impl EntryIntakeBatch {
    /// Move original facts exactly once, including failed authenticated frame loans.
    pub(crate) fn into_events(self) -> impl Iterator<Item = EntryIntakeEvent> {
        self.events.into_iter().flatten()
    }
}
/// Retain real server/client jobs; cancellation is distinct from checked completion.
pub(crate) struct ProductionEntryIntake {
    client: ExecutionClient,
    scope: Scope,
    jobs: JobSequence,
    original: EntrySetupBudget,
    server: CoordinatorEntryServerSession,
    initial_client: Option<CoordinatorEntryClientSession>,
    closed: bool,
    server_next: bool,
}
impl ProductionEntryIntake {
    /// Reserve the actual server only after existing claims/entry owner bind Serving.
    /// The caller retains its actual election, claims and registry owners throughout.
    /// # Errors
    /// Rejects missing current owned Serving facts or native admission before effects.
    pub(crate) fn new(
        client: ExecutionClient,
        scope: Scope,
        jobs: JobSequence,
        owner: &CoordinatorEntryOwner,
    ) -> Result<Self, Box<dyn Error>> {
        if owner.original_execution_owner() != Some(scope.owner_id()) {
            return Err(Box::new(EntryIntakeFailure::Identity));
        }
        current_owner(owner)?;
        let original = owner.original_setup_budget();
        let input = owner.initializing_server_input()?;
        let server = client.reserve_coordinator_entry_server(
            &scope,
            transport_spec(&jobs, MIN_ENTRY_SERVER_BYTES)?,
            input,
            original.until(),
        )?;
        Ok(Self {
            client,
            scope,
            jobs,
            original,
            server,
            initial_client: None,
            closed: false,
            server_next: true,
        })
    }

    /// Drain at most32 actual observations, alternating sides without losing facts.
    /// The original request becomes a real native self-client only after this server
    /// publishes actual activation. Authenticated frames are rechecked against the
    /// current retained owner and their fallible borrowed view before forwarding.
    /// This produces no Ready, launch permit, capture registration or durable seed.
    pub(crate) fn poll(&mut self, owner: &mut CoordinatorEntryOwner) -> EntryIntakeBatch {
        let mut events = std::array::from_fn(|_| None);
        for output in &mut events {
            let preferred = self.server_next;
            let next = self.take(preferred).or_else(|| self.take(!preferred));
            let Some((side, observation)) = next else {
                break;
            };
            self.server_next = side != EntrySide::Server;
            let failure = self.process(owner, side, &observation).err();
            if failure.is_some() {
                self.close();
            }
            *output = Some(EntryIntakeEvent { side, observation, failure });
        }
        self.server.control.rearm_observations();
        if let Some(client) = &self.initial_client {
            client.control.rearm_observations();
        }
        EntryIntakeBatch { events }
    }

    /// Borrow the actual jobs for the sole composition completion router.
    pub(crate) fn server(&mut self) -> &mut CoordinatorEntryServerSession {
        &mut self.server
    }
    /// Borrow the actual initial request job after real native admission.
    pub(crate) fn initial_client(&mut self) -> Option<&mut CoordinatorEntryClientSession> {
        self.initial_client.as_mut()
    }
    /// Fence both endpoints; their retained jobs still require terminal observation.
    pub(crate) fn close(&mut self) {
        self.closed = true;
        self.server.control.close();
        if let Some(client) = &self.initial_client {
            client.control.close();
        }
    }
    fn take(&self, server: bool) -> Option<(EntrySide, EntryTransportObservation)> {
        if server {
            self.server.control.take_observation().map(|fact| (EntrySide::Server, fact))
        } else {
            self.initial_client
                .as_ref()?
                .control
                .take_observation()
                .map(|fact| (EntrySide::InitialClient, fact))
        }
    }
    fn process(
        &mut self,
        owner: &mut CoordinatorEntryOwner,
        side: EntrySide,
        observation: &EntryTransportObservation,
    ) -> Result<(), EntryIntakeFailure> {
        if self.closed {
            return if matches!(observation, EntryTransportObservation::RequestAuthenticated { .. })
            {
                Err(EntryIntakeFailure::Owner(EntryAdmissionError::Closed))
            } else {
                Ok(())
            };
        }
        match (side, observation) {
            (
                EntrySide::Server,
                EntryTransportObservation::ConnectionRetired { connection, failure: Some(cause) },
            ) => {
                tracing::warn!(operation="entry_peer_disconnected",connection=?connection,cause=?cause,
                    "Entry requester disconnected before its reply was delivered");
            }
            (EntrySide::Server, EntryTransportObservation::Prepared) => {
                let known = current_owner(owner)?;
                owner.initializing_server_input().map_err(EntryIntakeFailure::Owner)?;
                self.server
                    .control
                    .activate_server(known, self.original.until())
                    .map_err(EntryIntakeFailure::Transport)?;
            }
            (EntrySide::Server, EntryTransportObservation::ServerActivated { owner: actual }) => {
                if Some(*actual) != owner.snapshot().owner || self.initial_client.is_some() {
                    return Err(EntryIntakeFailure::Identity);
                }
                current_owner(owner)?;
                let input = owner.client_input().map_err(EntryIntakeFailure::Owner)?;
                let spec = transport_spec(&self.jobs, MIN_ENTRY_CLIENT_BYTES)
                    .map_err(EntryIntakeFailure::Sequence)?;
                self.initial_client = Some(
                    self.client
                        .reserve_coordinator_entry_client(&self.scope, spec, input)
                        .map_err(EntryIntakeFailure::Admission)?,
                );
            }
            (
                EntrySide::Server,
                EntryTransportObservation::RequestAuthenticated { request, .. },
            ) => {
                let known = current_owner(owner)?;
                owner
                    .validate_current_owner(known, request.peer())
                    .map_err(EntryIntakeFailure::Owner)?;
                let now = self.client.now().map_err(EntryIntakeFailure::Clock)?;
                if now < request.received_at() || now >= request.until() {
                    return Err(EntryIntakeFailure::Transport(
                        EntryTransportError::InvalidDeadline,
                    ));
                }
                request.view().map_err(EntryIntakeFailure::Transport)?;
            }
            _ => {}
        }
        Ok(())
    }
}
impl Drop for ProductionEntryIntake {
    fn drop(&mut self) {
        self.close();
    }
}

fn current_owner(
    owner: &CoordinatorEntryOwner,
) -> Result<sugarsubstitute_contracts::lifecycle::CoordinatorKey, EntryIntakeFailure> {
    let actual = owner.snapshot();
    if actual.phase != EntryPhase::Serving {
        return Err(EntryIntakeFailure::Identity);
    }
    actual.owner.ok_or(EntryIntakeFailure::Identity)
}
fn transport_spec(
    jobs: &JobSequence,
    bytes: u64,
) -> Result<JobSpec, sugarsubstitute_contracts::execution::IdentityError> {
    Ok(JobSpec {
        id: jobs.allocate()?,
        operation: Operation::NativeLifecycle,
        class: JobClass::NativeObservation,
        deadline: None,
        reservation_bytes: bytes,
        replacement: None,
    })
}

#[cfg(test)]
#[path = "entry/tests.rs"]
mod tests;
