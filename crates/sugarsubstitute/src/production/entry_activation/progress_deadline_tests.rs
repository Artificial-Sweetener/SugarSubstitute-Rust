// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real activity arming method; native intake and cleanup effects are not run.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use sugarsubstitute_contracts::{execution::*, lifecycle::*};
use sugarsubstitute_execution::ExecutionOwner;

struct FixedClock(AtomicU64);
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Wake;
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        Ok(())
    }
}
fn moment(seconds: u64) -> Moment {
    Moment::from_nanos(seconds * 1_000_000_000)
}
fn owner() -> Result<Box<ProductionActivation>> {
    ProductionActivation::new(
        r"E:\unopened\sugarsubstitute.exe".into(),
        r"E:\unopened".into(),
        r"D:\unopened".into(),
    )
}
fn retained_record(until: Moment) -> Result<RetainedRecord> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    Ok(RetainedRecord {
        record: Box::new(Record {
            qualification: Default::default(),
            run,
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
            connection: ConnectionId::new(1)?,
            projected: None,
            attach: None,
            attached_memory: None,
            attach_failure: None,
            released_attach: None,
            attach_retirement: None,
            start: None,
            profile_predecessor: None,
            parent: None,
            retained_causes: None,
            preparation: None,
            retired: None,
            forwarded: None,
            ui_suspected: false,
            replied: false,
            closed: true,
            seed: incident_seed::SeedPreparation::new(),
            seeded: false,
            collection: incident_collection::IncidentCollection::new(),
            failed: None,
            failed_cause: None,
            failed_shutdown: None,
            failed_family_adopted: false,
            pending_desktop_failure: Some(incident_reporting::PendingDesktopFailure::new(
                Box::new(std::io::Error::other("retained original classification cause")),
                sugarsubstitute_application::incident::IncidentOperationBudget::new(moment(0))?,
                Some(until),
            )),
            restart: restart::RestartState::new(),
        }),
        _storage: crate::production::coordinator_backend::tests::storage_tests::claim(size_of::<
            Record,
        >()
            as u64)
        .map_err(outer)?,
    })
}

#[test]
fn activity_arming_retained_past_owner_and_resource_cannot_hide_other_backend_grace() -> Result<()>
{
    let executor = ExecutionOwner::new(
        Arc::new(FixedClock(AtomicU64::new(moment(20).as_nanos()))),
        Arc::new(Wake),
    )?;
    let client = executor.client();
    let scopes = [
        client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
    ];
    let mut activation = owner()?;
    activation.records[0] = Some(retained_record(moment(19))?);
    let original = [ShutdownBudget::new(moment(0))?, ShutdownBudget::new(moment(20))?];
    activation.backend = Some(
        crate::production::coordinator_backend::deadline_test_fixture::retained_cleanup(
            scopes, original,
        )
        .map_err(outer)?,
    );
    let before: Vec<_> =
        activation.backend.as_ref().ok_or("Backend absent")?.backend.deadlines().collect();
    assert_eq!(
        before,
        vec![
            original[0].graceful_until,
            original[0].settle_until,
            original[1].graceful_until,
            original[1].settle_until
        ]
    );
    let cause = std::ptr::from_ref(
        activation.records[0]
            .as_ref()
            .ok_or("Record absent")?
            .record
            .pending_desktop_failure
            .as_ref()
            .ok_or("Cause absent")?,
    );
    let anchor = Instant::now();
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(activation.next_wake, anchor.checked_add(Duration::from_secs(10)));
    assert_eq!(
        activation
            .backend
            .as_ref()
            .ok_or("Backend absent")?
            .backend
            .deadlines()
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(
        std::ptr::from_ref(
            activation.records[0]
                .as_ref()
                .ok_or("Record absent")?
                .record
                .pending_desktop_failure
                .as_ref()
                .ok_or("Cause absent")?
        ),
        cause
    );
    assert_eq!(
        activation.records[0]
            .as_ref()
            .ok_or("Record absent")?
            .record
            .pending_desktop_failure
            .as_ref()
            .ok_or("Cause absent")?
            .until(),
        moment(19)
    );
    Ok(())
}

#[test]
fn activity_arming_selects_unchanged_total_after_due_grace_without_renewing_or_marking_it()
-> Result<()> {
    let clock = Arc::new(FixedClock(AtomicU64::new(moment(20).as_nanos())));
    let executor = ExecutionOwner::new(clock.clone(), Arc::new(Wake))?;
    let client = executor.client();
    let scopes = [
        client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
    ];
    let mut activation = owner()?;
    let original = [ShutdownBudget::new(moment(20))?, ShutdownBudget::new(moment(0))?];
    activation.backend = Some(
        crate::production::coordinator_backend::deadline_test_fixture::retained_cleanup(
            scopes, original,
        )
        .map_err(outer)?,
    );
    let before: Vec<_> =
        activation.backend.as_ref().ok_or("Backend absent")?.backend.deadlines().collect();
    let anchor = Instant::now();
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(activation.next_wake, anchor.checked_add(Duration::from_secs(10)));
    clock.0.store(moment(30).as_nanos(), Ordering::Release);
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(activation.next_wake, anchor.checked_add(Duration::from_secs(5)));
    assert_eq!(
        activation
            .backend
            .as_ref()
            .ok_or("Backend absent")?
            .backend
            .deadlines()
            .collect::<Vec<_>>(),
        before
    );
    // The due owner still owes its finite force turn; selection alone changes no actor state.
    clock.0.store(moment(35).as_nanos(), Ordering::Release);
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(activation.next_wake, None);
    assert_eq!(
        activation
            .backend
            .as_ref()
            .ok_or("Backend absent")?
            .backend
            .deadlines()
            .collect::<Vec<_>>(),
        before
    );
    Ok(())
}

#[test]
fn activity_arming_empty_owner_clears_previous_native_wake() -> Result<()> {
    let executor = ExecutionOwner::new(
        Arc::new(FixedClock(AtomicU64::new(moment(20).as_nanos()))),
        Arc::new(Wake),
    )?;
    let mut activation = owner()?;
    let anchor = Instant::now();
    activation.next_wake = Some(anchor);
    activation.arm_next_wake(&executor.client(), anchor)?;
    assert_eq!(activation.next_wake, None);
    Ok(())
}
