// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject foreign or stopped sources before effects and retain actual held creation credit through join.
use super::*;
use crate::{
    backend_custody::{BackendCustodyParentStorage, BackendCustodyPipeSession},
    recovery_supervisor::RecoveryPipeSession,
};

/// Real authenticated archive, once-only grant and actual execution-owned native boundary sessions.
struct Fixture {
    rig: Rig,
    entry_scope: Scope,
    entry: CoordinatorEntryServerSession,
    source: RegisteredStartupSource,
    completed: Completed,
    grant: RegisteredRecoveryGrant,
    custody_scope: Scope,
    custody: BackendCustodyPipeSession,
    parent: BackendCustodyParentStorage,
    recovery_scope: Scope,
    recovery: RecoveryPipeSession,
}
fn fixture() -> TestResult<Fixture> {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (entry_scope, entry) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &entry)?)?;
    drop(memory);
    let mut completed = issued(&rig, &source)?;
    let snapshot = take(&mut completed)?;
    let grant =
        rig.client.issue_registered_recovery_grant(snapshot, &source, &completed, key()?)?;
    let (custody_scope, custody, parent) = recovery_parent(&rig, 3, 4)?;
    let (recovery_scope, recovery) = recovery_server(&rig, 4, 5, &grant)?;
    Ok(Fixture {
        rig,
        entry_scope,
        entry,
        source,
        completed,
        grant,
        custody_scope,
        custody,
        parent,
        recovery_scope,
        recovery,
    })
}
/// Join real workers with only the original cancelled completion retained, then dispose its credit.
fn finish(
    mut fixture: Fixture,
    completion: JobCompletion<ProcessCreationReport, ProcessCreationFailure>,
) -> TestResult {
    fixture.recovery.control.close();
    drop(observed_stage(
        "recovery pipe retirement",
        fixture.rig.completion(&mut fixture.recovery.completion),
    )?);
    fixture.custody.control.close();
    drop(observed_stage(
        "custody pipe retirement",
        fixture.rig.completion(&mut fixture.custody.completion),
    )?);
    observed_stage("Root Entry retirement", close(&fixture.rig, &mut fixture.entry))?;
    let core = fixture.rig.client.core.clone();
    let Fixture {
        rig,
        entry_scope,
        entry,
        source,
        completed,
        grant,
        custody_scope,
        custody,
        parent,
        recovery_scope,
        recovery,
    } = fixture;
    drop((
        entry_scope,
        entry,
        source,
        completed,
        grant,
        custody_scope,
        custody,
        parent,
        recovery_scope,
        recovery,
    ));
    let Rig { owner, client, clock, wake, scope } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, PROCESS_CREATION_STORAGE_BYTES);
    assert_eq!(lock(&core.state).bytes, PROCESS_CREATION_STORAGE_BYTES);
    drop(completion);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(lock(&core.state).bytes, 0);
    drop((client, clock, wake, scope));
    Ok(())
}
#[test]
fn recovery_held_creation_rejects_foreign_core_and_equal_key_foreign_parent_without_consuming_grant()
-> TestResult {
    let fixture = fixture()?;
    let foreign = Rig::new()?;
    let (_foreign_scope, mut foreign_custody, foreign_parent) = recovery_parent(&foreign, 2, 1)?;
    assert_eq!(foreign_parent.key(), fixture.parent.key());
    let effects = Arc::new(Effects::default());
    let factories = Arc::new(AtomicUsize::new(0));
    let mut input = Some(Input(effects.clone()));
    let original_bytes = fixture.rig.scope.snapshot().reserved_bytes;
    let make = factories.clone();
    let rejected = foreign.client.reserve_recovery_custody_creation(
        &foreign.scope,
        spec_for(2, &fixture.grant)?,
        &fixture.parent,
        &fixture.grant,
        &mut input,
        move |input, _, _| {
            make.fetch_add(1, Ordering::AcqRel);
            Ok(Box::new(Driver(input)))
        },
    );
    assert!(rejected.is_err());
    assert!(input.as_ref().is_some_and(|input| Arc::ptr_eq(&input.0, &effects)));
    assert_eq!(foreign.scope.snapshot().reserved_bytes, 0);
    assert_eq!(fixture.rig.scope.snapshot().reserved_bytes, original_bytes);
    assert!(lock(&fixture.grant.state()?.stages).creation.is_none());
    let make = factories.clone();
    let rejected = fixture.rig.client.reserve_recovery_custody_creation(
        &fixture.rig.scope,
        spec_for(6, &fixture.grant)?,
        &foreign_parent,
        &fixture.grant,
        &mut input,
        move |input, _, _| {
            make.fetch_add(1, Ordering::AcqRel);
            Ok(Box::new(Driver(input)))
        },
    );
    assert!(rejected.is_err());
    assert!(input.as_ref().is_some_and(|input| Arc::ptr_eq(&input.0, &effects)));
    assert_eq!(fixture.rig.scope.snapshot().reserved_bytes, original_bytes);
    assert!(lock(&fixture.grant.state()?.stages).creation.is_none());
    assert_eq!(factories.load(Ordering::Acquire), 0);
    assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
    assert_eq!(effects.retired.load(Ordering::Acquire), 0);
    assert_eq!(effects.dropped.load(Ordering::Acquire), 0);
    fixture.parent.check()?;
    fixture.grant.check()?;
    let make = factories.clone();
    let permit = fixture.rig.client.reserve_recovery_custody_creation(
        &fixture.rig.scope,
        spec_for(6, &fixture.grant)?,
        &fixture.parent,
        &fixture.grant,
        &mut input,
        move |input, _, _| {
            make.fetch_add(1, Ordering::AcqRel);
            Ok(Box::new(Driver(input)))
        },
    )?;
    assert!(input.is_none());
    assert_eq!(
        fixture.rig.scope.snapshot().reserved_bytes,
        original_bytes + PROCESS_CREATION_STORAGE_BYTES
    );
    let mut job = permit.cancel_and_observe();
    let completion = observed_stage("creation terminal", fixture.rig.completion(&mut job))?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert_eq!(completion.spec, spec_for(6, &fixture.grant)?);
    assert_eq!(completion.memory.bytes(), PROCESS_CREATION_STORAGE_BYTES);
    assert_eq!(factories.load(Ordering::Acquire), 0);
    assert_eq!(effects.dropped.load(Ordering::Acquire), 1);
    assert!(!lock(&fixture.grant.state()?.stages).committed);
    assert_eq!(lock(&fixture.grant.state()?.stages).creation, Some(JobId::new(6)?));
    let mut replay = Some(Input(effects.clone()));
    let make = factories.clone();
    assert!(
        fixture
            .rig
            .client
            .reserve_recovery_custody_creation(
                &fixture.rig.scope,
                spec_for(7, &fixture.grant)?,
                &fixture.parent,
                &fixture.grant,
                &mut replay,
                move |input, _, _| {
                    make.fetch_add(1, Ordering::AcqRel);
                    Ok(Box::new(Driver(input)))
                },
            )
            .is_err()
    );
    assert!(replay.is_some());
    assert_eq!(factories.load(Ordering::Acquire), 0);
    drop(replay);
    drop(job);
    foreign_custody.control.close();
    drop(observed_stage(
        "foreign custody pipe retirement",
        foreign.completion(&mut foreign_custody.completion),
    )?);
    drop(foreign_parent);
    drop(foreign_custody);
    drop(foreign);
    finish(fixture, completion)
}
#[test]
fn recovery_held_creation_exact_ack_cannot_reopen_cancelled_root_or_closed_grant() -> TestResult {
    for close_grant in [false, true] {
        let fixture = fixture()?;
        let effects = Arc::new(Effects::default());
        let factories = Arc::new(AtomicUsize::new(0));
        let mut input = Some(Input(effects.clone()));
        let make = factories.clone();
        let permit = fixture.rig.client.reserve_recovery_custody_creation(
            &fixture.rig.scope,
            spec_for(6, &fixture.grant)?,
            &fixture.parent,
            &fixture.grant,
            &mut input,
            move |input, _, _| {
                make.fetch_add(1, Ordering::AcqRel);
                Ok(Box::new(Driver(input)))
            },
        )?;
        assert!(input.is_none());
        assert_eq!(factories.load(Ordering::Acquire), 0);
        assert_eq!(lock(&fixture.grant.state()?.stages).creation, Some(JobId::new(6)?));
        assert!(!lock(&fixture.grant.state()?.stages).committed);
        if close_grant {
            fixture.grant.close_admission();
        } else {
            fixture.rig.client.core.cancel_scope(fixture.entry_scope.id());
        }
        assert!(fixture.grant.check().is_err());
        let rejected = permit
            .begin(ack(fixture.grant.key())?)
            .err()
            .ok_or("Original Root stop was reversed by exact ACK")?;
        assert!(!lock(&fixture.grant.state()?.stages).committed);
        assert_eq!(rejected.permit.key(), fixture.grant.key());
        let mut job = rejected.permit.cancel_and_observe();
        let completion = observed_stage("creation terminal", fixture.rig.completion(&mut job))?;
        assert!(matches!(completion.outcome, JobOutcome::Cancelled));
        assert_eq!(completion.spec, spec_for(6, &fixture.grant)?);
        assert_eq!(completion.memory.bytes(), PROCESS_CREATION_STORAGE_BYTES);
        assert_eq!(factories.load(Ordering::Acquire), 0);
        assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
        assert_eq!(effects.retired.load(Ordering::Acquire), 0);
        assert_eq!(effects.dropped.load(Ordering::Acquire), 1);
        assert_eq!(lock(&fixture.grant.state()?.stages).creation, Some(JobId::new(6)?));
        assert!(!lock(&fixture.grant.state()?.stages).committed);
        drop(job);
        finish(fixture, completion)?;
    }
    Ok(())
}

#[path = "admission/preack_pair.rs"]
mod preack_pair;
