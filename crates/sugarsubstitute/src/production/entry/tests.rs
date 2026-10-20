// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject inert/revoked ownership and type-check actual session handoffs.
use super::*;
use sugarsubstitute_contracts::{
    configuration::{
        ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration,
        Theme,
    },
    coordinator_entry::EntryLaunchRequest,
    execution::Moment,
};
use sugarsubstitute_supervision::coordinator_entry::CoordinatorEntryInput;

fn inert_owner() -> Result<CoordinatorEntryOwner, Box<dyn Error>> {
    let path = std::path::PathBuf::from(r"E:\inert\entry");
    let config = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
        ProfileId::new("original".into())?,
        Presentation::new("en", Theme::Dark, false)?,
    );
    // Negative inert-input tests acquire no namespace, peer or authenticated frame.
    let input = EntryLaunchRequest::new(
        config,
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        vec![65, 61, 66, 0, 0],
    )?;
    Ok(CoordinatorEntryOwner::new(
        CoordinatorEntryInput::new(
            path.clone(),
            path,
            sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation::Account,
            input,
        )?,
        EntrySetupBudget::new(Moment::from_nanos(7))?,
    ))
}

#[test]
fn inert_and_revoked_owner_cannot_supply_serving_admission() -> Result<(), Box<dyn Error>> {
    let mut owner = inert_owner()?;
    assert!(matches!(current_owner(&owner), Err(EntryIntakeFailure::Identity)));
    let original = owner.original_setup_budget();
    assert_eq!(original.started(), Moment::from_nanos(7));
    assert_eq!(original.until(), Moment::from_nanos(3_600_000_000_007));
    owner.close_admission();
    assert!(matches!(current_owner(&owner), Err(EntryIntakeFailure::Identity)));
    assert_eq!(owner.original_setup_budget(), original);
    Ok(())
}

#[test]
fn actual_intake_rejects_inert_and_closed_owner_before_allocating_transport_jobs()
-> Result<(), Box<dyn Error>> {
    use std::sync::Arc;
    use sugarsubstitute_contracts::execution::{
        Clock, CompletionWake, ExecutionError, ScopeId, ScopeSpec, WaitFuture,
    };
    struct ClockPort;
    impl Clock for ClockPort {
        fn now(&self) -> Moment {
            Moment::from_nanos(7)
        }
        fn sleep_until(&self, _: Moment) -> WaitFuture {
            Box::pin(std::future::pending())
        }
    }
    struct Wake;
    impl CompletionWake for Wake {
        fn wake(&self) -> Result<(), ExecutionError> {
            Ok(())
        }
    }
    let execution =
        sugarsubstitute_execution::ExecutionOwner::new(Arc::new(ClockPort), Arc::new(Wake))?;
    let client = execution.client();
    let mut owner = inert_owner()?;
    let original = owner.original_setup_budget();
    for closed in [false, true] {
        let scope = client.create_native_scope(
            ScopeSpec { id: ScopeId::new(if closed { 2 } else { 1 })?, deadline: None },
            None,
        )?;
        if closed {
            owner.close_admission();
        }
        let jobs = JobSequence::new();
        let result = ProductionEntryIntake::new(client.clone(), scope, jobs.clone(), &owner);
        let Err(cause) = result else {
            return Err("Inert owner opened entry endpoint".into());
        };
        assert!(cause.downcast_ref::<EntryIntakeFailure>().is_some());
        assert_eq!(jobs.allocate()?.get(), 1);
        assert_eq!(owner.original_setup_budget(), original);
    }
    Ok(())
}

#[test]
fn transport_jobs_share_original_sequence_and_persistent_native_class() -> Result<(), Box<dyn Error>>
{
    let sequence = JobSequence::new();
    let server = transport_spec(&sequence, MIN_ENTRY_SERVER_BYTES)?;
    let client = transport_spec(&sequence, MIN_ENTRY_CLIENT_BYTES)?;
    assert_eq!((server.id.get(), client.id.get()), (1, 2));
    for (spec, required) in [(server, MIN_ENTRY_SERVER_BYTES), (client, MIN_ENTRY_CLIENT_BYTES)] {
        assert_eq!(spec.operation, Operation::NativeLifecycle);
        assert_eq!(spec.class, JobClass::NativeObservation);
        assert_eq!(spec.deadline, None);
        assert_eq!(spec.reservation_bytes, required);
        assert_eq!(spec.replacement, None);
    }
    Ok(())
}

#[test]
fn bounded_stack_batch_moves_original_metadata_and_failure_once() -> Result<(), Box<dyn Error>> {
    let mut events = std::array::from_fn(|_| None);
    events[0] = Some(EntryIntakeEvent {
        side: EntrySide::Server,
        observation: EntryTransportObservation::Prepared,
        failure: Some(EntryIntakeFailure::Identity),
    });
    events[31] = Some(EntryIntakeEvent {
        side: EntrySide::InitialClient,
        observation: EntryTransportObservation::Prepared,
        failure: None,
    });
    let mut moved = EntryIntakeBatch { events }.into_events();
    let first = moved.next().ok_or("First original fact absent")?;
    assert_eq!(first.side, EntrySide::Server);
    assert!(matches!(first.observation, EntryTransportObservation::Prepared));
    assert!(matches!(first.failure, Some(EntryIntakeFailure::Identity)));
    let last = moved.next().ok_or("Last original fact absent")?;
    assert_eq!(last.side, EntrySide::InitialClient);
    assert!(last.failure.is_none());
    assert!(moved.next().is_none());
    Ok(())
}

// These exact function types compile the held source boundary; they do not stand
// in for native success, authenticated peer/frame creation or actual settlement.
const _: fn(&mut ProductionEntryIntake, &mut CoordinatorEntryOwner) -> EntryIntakeBatch =
    ProductionEntryIntake::poll;
const _: fn(&mut ProductionEntryIntake) -> &mut CoordinatorEntryServerSession =
    ProductionEntryIntake::server;
const _: fn(&mut ProductionEntryIntake) -> Option<&mut CoordinatorEntryClientSession> =
    ProductionEntryIntake::initial_client;
