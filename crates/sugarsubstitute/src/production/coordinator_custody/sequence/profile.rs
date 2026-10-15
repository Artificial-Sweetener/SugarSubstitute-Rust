// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain real profile jobs before Finish while independently observing transport failures.
use super::*;
use sugarsubstitute_contracts::{execution::*, lifecycle::*};
use sugarsubstitute_supervision::{
    WindowsProcessRegistry,
    profiles::{PROFILE_OPERATION_BYTES, WindowsProfileLease},
};

fn spec(id: u64, bytes: u64) -> std::result::Result<JobSpec, Box<dyn Error + Send + Sync>> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(30_000_000_000)),
        reservation_bytes: bytes,
        replacement: None,
    })
}

#[test]
fn actual_released_pending_profile_blocks_finish_until_original_job_consumption() -> TestResult {
    let fixture = Fixture::new(None)?;
    let key = key()?;
    let rejection = WindowsProcessRegistry::submit_new(
        &fixture.client,
        &fixture.scope,
        spec(1, WindowsProcessRegistry::required_reservation_bytes())?,
        key.owner(),
    )
    .err()
    .ok_or("Native-only scope admitted finite registry construction")?;
    assert!(
        matches!(
            rejection.source().and_then(|cause| cause.downcast_ref::<AdmissionError>()),
            Some(AdmissionError::WrongClass {
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle
            })
        ),
        "Original unexpected registry rejection: source={:?}",
        rejection.source()
    );
    let profile_scope =
        fixture.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let mut job = WindowsProcessRegistry::submit_new(
        &fixture.client,
        &profile_scope,
        spec(1, WindowsProcessRegistry::required_reservation_bytes())?,
        key.owner(),
    )?;
    fixture.terminal(job.spec().id)?;
    let registry = job.try_take_completion().ok_or("Actual registry outcome absent")?;
    let JobOutcome::Success(value) = &registry.outcome else {
        return Err("Actual registry failed".into());
    };
    let run = RunKey {
        owner: key.owner(),
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let mut profile = WindowsProfileLease::new(
        run,
        std::path::PathBuf::from(r"D:\unopened-retirement-profile"),
        value,
    )?;
    let release = profile.submit_release(
        &fixture.client,
        &profile_scope,
        spec(2, PROFILE_OPERATION_BYTES)?,
        value,
    )?;
    let batch = fixture.terminal(release)?;
    assert!(profile.snapshot().released && profile.snapshot().pending);
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let mut owner = Retirement::default();
    let mut fixture = fixture;
    owner.advance(
        budget,
        true,
        profile.snapshot().retirement_ready(),
        fixture.client.now()?,
        key,
        &mut fixture.port,
    )?;
    assert_eq!(fixture.port.events, vec![Event::Observe]);
    assert!(!owner.finish_received && !owner.pipe_closed && !owner.settled);
    let parent_storage = crate::production::coordinator_backend::tests::storage_tests::claim(
        crate::production::parent_lifecycle::ParentLifecycle::storage_bytes(),
    )?;
    let mut desktop_parent = crate::production::parent_lifecycle::ParentLifecycle::new(
        run,
        sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        sugarsubstitute_contracts::configuration::ProfileId::new("Retire_1".into())?,
        ConnectionId::new(1)?,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            30_000_000_000,
            1_000_000_000,
        )?,
        &mut Some(crate::production::parent_lifecycle::causes::BackendCauses::new(parent_storage)?),
    )?;
    assert!(desktop_parent.profile_completion(&batch, &mut profile)?.is_none());
    assert!(profile.snapshot().retirement_ready());
    owner.advance(
        budget,
        true,
        profile.snapshot().retirement_ready(),
        fixture.client.now()?,
        key,
        &mut fixture.port,
    )?;
    assert_eq!(fixture.port.events, vec![Event::Observe, Event::FinishPending]);
    assert_eq!(owner.original, Some(budget));
    drop(profile);
    drop(registry);
    drop(job);
    Ok(())
}

#[test]
fn profile_pending_keeps_original_transport_failure_and_actual_pipe_retirement_observable()
-> TestResult {
    let mut fixture = Fixture::new(Some(83))?;
    let key = fixture.port.key()?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let cause = sugarsubstitute_execution::windows::NativeObservationError::new(
        "profile_pending_transport",
        std::io::Error::from_raw_os_error(109),
    );
    let pointer = std::ptr::from_ref(cause.source().ok_or("Original source absent")?);
    fixture.port.failure = Some(cause);
    fixture.port.family_retired = true;
    let mut owner = Retirement::default();
    let failure = owner
        .advance(budget, true, false, fixture.client.now()?, key, &mut fixture.port)
        .err()
        .ok_or("Original transport error lost")?;
    assert_eq!(std::ptr::from_ref(failure.source().ok_or("Original source replaced")?), pointer);
    assert!(fixture.port.events.starts_with(&[Event::Observe, Event::Stop, Event::PipeClosed]));
    assert!(!owner.finish_received);
    assert_eq!(owner.original, Some(budget));
    drop(owner);
    fixture.close_fixture()?;
    Ok(())
}
