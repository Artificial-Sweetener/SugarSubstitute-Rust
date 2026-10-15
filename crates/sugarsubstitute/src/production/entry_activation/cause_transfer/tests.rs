// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real cause/claim/actor ownership through the exact replacement and construction owners.
//! Native bootstrap is the external boundary; these shape-only constructor facts confer no native authority.
use super::*;
use crate::production::parent_lifecycle::SettledAttemptFailure;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::{
    execution::Generation,
    lifecycle::*,
    startup::{LaunchId, MainIdentity},
};
#[derive(Debug)]
struct Tracked(Arc<AtomicUsize>);
impl std::fmt::Display for Tracked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Original tracked cause")
    }
}
impl Error for Tracked {}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn record(generation: u64) -> Result<Record> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(generation)?,
    };
    Ok(Record {
        qualification: Default::default(),
        run,
        launch: LaunchId::new(generation)?,
        navigation: Generation::new(generation)?,
        connection: ConnectionId::new(generation)?,
        projected: None,
        attach: None,
        attached_memory: None,
        attach_failure: None,
        released_attach: None,
        attach_retirement: None,
        start: None,
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
        pending_desktop_failure: None,
        restart: restart::RestartState::new(),
    })
}
#[test]
fn replacing_prelaunch_record_then_constructing_parent_moves_same_cause_box_and_original_claim()
-> Result<()> {
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut previous = record(1)?;
    let mut current = record(2)?;
    let charge = crate::production::coordinator_backend::tests::storage_tests::claim(
        ParentLifecycle::storage_bytes(),
    )
    .map_err(outer)?;
    let mut owner = BackendCauses::new(charge)?;
    let cause = Box::new(Tracked(dropped.clone()));
    let original = std::ptr::from_ref(cause.as_ref());
    let identity = MainIdentity {
        run: previous.run,
        launch: previous.launch,
        navigation: previous.navigation,
    };
    owner.insert_for_owner_test(SettledAttemptFailure {
        identity,
        resource: ResourceKey {
            owner: previous.run.owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
        cause: LifecycleFailure::new(LifecycleFailureCode::UnexpectedExit, cause),
    })?;
    let allocation = owner.allocation();
    let reserved = owner.claim().reserved_bytes();
    previous.retained_causes = Some(owner);
    before_replace(&mut previous, &mut current, true)?;
    drop(previous);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    assert_eq!(
        current.retained_causes.as_ref().ok_or("Record lost cause owner")?.allocation(),
        allocation
    );
    let parent = parent_construction::construct_with_causes(
        current.run,
        current.launch,
        sugarsubstitute_contracts::configuration::ProfileId::new("Cause_1".into())?,
        current.connection,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            100,
            1_000_000_000,
        )?,
        &mut current.retained_causes,
    )?;
    assert!(current.retained_causes.is_none());
    assert_eq!(parent.cause_allocation_for_owner_test()?, allocation);
    assert_eq!(parent.cause_reserved_for_owner_test()?, reserved);
    let retained = parent.settled_attempt_failure().ok_or("Original failure lost")?;
    assert_eq!(retained.identity, identity);
    assert!(std::ptr::eq(
        retained
            .cause
            .source()
            .and_then(|cause| cause.downcast_ref::<Tracked>())
            .ok_or("Original cause type lost")?,
        original
    ));
    drop(current);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    drop(parent);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    Ok(())
}
