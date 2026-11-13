// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed metadata reflects actual owner admission rather than a caller specification.
use super::*;

#[test]
fn process_permit_metadata_retains_inherited_deadline_and_actual_owner_on_cancel() -> TestResult {
    let rig = Rig::new()?;
    let ancestor = rig.client.create_scope(
        ScopeSpec { id: ScopeId::new(2)?, deadline: Some(Moment::from_nanos(100)) },
        Some(&rig.scope),
    )?;
    let child = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(3)?, deadline: Some(Moment::from_nanos(200)) },
        Some(&ancestor),
        true,
    )?;
    let mut request = spec(1, JobClass::NativeObservation, 4096)?;
    request.operation = Operation::NativeLifecycle;
    request.deadline = Some(Moment::from_nanos(300));
    let permit = reserve(&rig.client.core, &child, request, NativeCorrelation::new(1)?)?;
    assert_eq!(permit.owner_id(), child.owner_id());
    assert_eq!(permit.deadline(), Some(Moment::from_nanos(100)));
    child.cancel();
    assert_eq!(permit.deadline(), Some(Moment::from_nanos(100)));
    let mut job = permit.cancel_input();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn process_permit_foreign_owner_is_distinct_and_bind_preserves_original_deadline() -> TestResult {
    let rig = Rig::new()?;
    let foreign = Rig::new()?;
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: Some(Moment::from_nanos(500)) },
        None,
        true,
    )?;
    let mut request = spec(1, JobClass::NativeObservation, 4096)?;
    request.operation = Operation::NativeLifecycle;
    request.deadline = Some(Moment::from_nanos(600));
    let permit = reserve(&rig.client.core, &scope, request, NativeCorrelation::new(17)?)?;
    assert_ne!(permit.owner_id(), foreign.scope.owner_id());
    assert_eq!(permit.deadline(), Some(Moment::from_nanos(500)));
    let boundary = Boundary::new(false, false);
    let mut job = permit.bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    assert_eq!(job.spec().deadline, Some(Moment::from_nanos(600)));
    rig.clock.advance(500);
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
