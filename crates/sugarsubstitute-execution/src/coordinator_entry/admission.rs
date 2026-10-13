// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Charge one persistent native session before endpoint construction or request storage allocation.
use super::*;
use crate::{
    admission::RequestKind,
    task::{JobHandle, QueuedJob, Work},
};
use sugarsubstitute_contracts::execution::{AdmissionError, JobSpec, OwnedJob, Scope};

pub(crate) struct EntryWork {
    pub(super) shared: Arc<Shared>,
    pub(super) job: Arc<Job<EntryTransportReport, EntryTransportError>>,
    pub(super) lease: Option<Box<dyn EntryLease>>,
    pub(super) input: Option<Box<CoordinatorEntryClientInput>>,
}
fn persistent(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    minimum: u64,
) -> Result<(), EntryTransportAdmissionError> {
    if scope.owner_id() != core.id {
        return Err(EntryTransportAdmissionError::Admission(AdmissionError::ForeignScope {
            expected_owner: core.id,
            actual_owner: scope.owner_id(),
        }));
    }
    if spec.deadline.is_some()
        || lock(&core.state).scopes.get(&scope.id()).is_some_and(|scope| scope.deadline.is_some())
    {
        return Err(EntryTransportAdmissionError::ConflictingDeadline);
    }
    if spec.reservation_bytes < minimum {
        return Err(EntryTransportAdmissionError::Admission(
            AdmissionError::NativeReservationTooSmall {
                requested_bytes: spec.reservation_bytes,
                minimum_bytes: minimum,
            },
        ));
    }
    Ok(())
}
pub(crate) fn reserve_server(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    input: CoordinatorEntryServerInput,
    original_preparation: Moment,
    lease: Box<dyn EntryLease>,
) -> Result<CoordinatorEntryServerSession, EntryTransportAdmissionError> {
    persistent(core, scope, spec, MIN_ENTRY_SERVER_BYTES)?;
    let now = core.clock.now();
    if original_preparation <= now {
        return Err(EntryTransportAdmissionError::Admission(AdmissionError::DeadlineExpired {
            deadline: original_preparation,
            now,
        }));
    }
    let remaining = original_preparation.checked_duration_since(now).map_err(|cause| {
        EntryTransportAdmissionError::Clock(NativeObservationError::new(
            "entry_preparation_remaining",
            cause,
        ))
    })?;
    if remaining > std::time::Duration::from_secs(30) {
        return Err(EntryTransportAdmissionError::Clock(NativeObservationError::new(
            "entry_preparation_horizon",
            EntryTransportError::InvalidDeadline,
        )));
    }
    let facts = *input.facts();
    let (control, completion) = reserve(
        core,
        scope,
        spec,
        Reservation { server: true, facts, original_preparation, input: None, lease },
    )?;
    Ok(CoordinatorEntryServerSession { control, completion })
}
pub(crate) fn reserve_client(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    input: CoordinatorEntryClientInput,
    mut lease: Box<dyn EntryLease>,
) -> Result<CoordinatorEntryClientSession, EntryTransportAdmissionError> {
    persistent(core, scope, spec, MIN_ENTRY_CLIENT_BYTES)?;
    // Destination observation precedes the original final pre-effect check.
    // A failed destination sample still observes mandatory original controls.
    let sampled = core
        .clock
        .metadata()
        .map_err(|cause| NativeObservationError::new("entry_destination_counter_clock", cause));
    let original = if sampled.is_ok() {
        input.request.check_original_core(core)
    } else {
        input.request.observe_current_original_core(core).map(|()| None)
    };
    let (before, original_until) = match (sampled, original) {
        (Err(first), after) => {
            return Err(clock_rejection(&input.request, first, after.err()));
        }
        (Ok(_), Err(after)) => return Err(EntryTransportAdmissionError::Preparation(after)),
        (Ok(before), Ok(until)) => (before, until),
    };
    let conversion = lease.sample_counter().and_then(|sample| {
        let until = crate::counter_deadline::decode(before, sample, input.source_setup)?;
        validate_horizon(sample, input.source_setup)
            .map_err(|cause| NativeObservationError::new("entry_source_horizon", cause))?;
        if original_until.is_some_and(|original| until > original) {
            return Err(NativeObservationError::new(
                "entry_original_preparation_renewal",
                EntryTransportError::InvalidDeadline,
            ));
        }
        Ok(until)
    });
    let post = input.request.observe_current_original_core(core);
    let until = match (conversion, post) {
        (Err(first), after) => {
            return Err(clock_rejection(&input.request, first, after.err()));
        }
        (Ok(_), Err(after)) => return Err(EntryTransportAdmissionError::Preparation(after)),
        (Ok(until), Ok(_)) => until,
    };
    let facts = input.facts;
    let (control, completion) = reserve(
        core,
        scope,
        spec,
        Reservation {
            server: false,
            facts,
            original_preparation: until,
            input: Some(input),
            lease,
        },
    )?;
    Ok(CoordinatorEntryClientSession { control, completion })
}
fn clock_rejection(
    input: &crate::EntryRequestInput,
    first: NativeObservationError,
    after: Option<crate::EntryPreparationError>,
) -> EntryTransportAdmissionError {
    if input.original_owner().is_none() && after.is_none() {
        return EntryTransportAdmissionError::Clock(first);
    }
    EntryTransportAdmissionError::Prepared(EntryTransportError::prepared(
        EntryTransportError::Clock(first),
        after.map(EntryTransportError::Preparation),
        input.original_owner(),
    ))
}

struct Reservation {
    server: bool,
    facts: sugarsubstitute_contracts::coordinator_entry::EntryNativeIdentityFacts,
    original_preparation: Moment,
    input: Option<CoordinatorEntryClientInput>,
    lease: Box<dyn EntryLease>,
}
fn reserve(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    reservation: Reservation,
) -> Result<
    (CoordinatorEntryControl, OwnedJob<EntryTransportReport, EntryTransportError>),
    EntryTransportAdmissionError,
> {
    let Reservation { server, facts, original_preparation, input, lease } = reservation;
    core.admit(scope, spec, RequestKind::Native, |context, cancel, timing| {
        let admitted = timing.admitted;
        let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(core));
        let shared = Arc::new(Shared {
            server,
            facts,
            original_preparation,
            state: Mutex::new(State::new(server)),
            metadata: Mutex::new(None),
            core: Arc::downgrade(core),
            job_id: spec.id,
        });
        let work = EntryWork {
            shared: shared.clone(),
            job: job.clone(),
            lease: Some(lease),
            input: input.map(Box::new),
        };
        let queued = QueuedJob {
            context,
            job: job.clone(),
            work: Work::Native(crate::native::NativeWork::CoordinatorEntry(work)),
        };
        let control = CoordinatorEntryControl { shared, job: job.clone() };
        (queued, (control, OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job)))))
    })
    .map_err(EntryTransportAdmissionError::Admission)
}
pub(super) fn validate_horizon(
    sample: (u64, u64),
    source: sugarsubstitute_contracts::execution::CounterDeadline,
) -> Result<(), EntryTransportError> {
    let remaining = (source.ticks() as u64)
        .checked_sub(sample.0)
        .ok_or(EntryTransportError::InvalidDeadline)?;
    let horizon =
        u128::from(sample.1).checked_mul(30).ok_or(EntryTransportError::InvalidDeadline)?;
    if remaining == 0 || u128::from(remaining) > horizon {
        return Err(EntryTransportError::InvalidDeadline);
    }
    Ok(())
}
