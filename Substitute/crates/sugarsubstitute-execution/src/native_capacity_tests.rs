// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Keep native reservation and ordinary capacity independent through real admission transactions.

use super::*;

#[test]
fn reserved_native_queue_is_bounded_and_does_not_consume_ordinary_capacity() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let mut permits = Vec::new();
    for id in 1..=384 {
        permits.push(permit(&rig, &scope, id)?);
    }
    assert!(matches!(
        reserve(&rig.client.core, &scope, native_spec(385)?, NativeCorrelation::new(385)?),
        Err(AdmissionError::QueueSaturated { class: JobClass::NativeObservation })
    ));
    let mut ordinary =
        rig.client.submit_async(&rig.scope, spec(385, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, String>(())
        })?;
    assert!(matches!(rig.completion(&mut ordinary)?.outcome, JobOutcome::Success(())));
    assert_eq!(scope.snapshot().queued, 384);
    assert_eq!(scope.snapshot().reserved_bytes, 384 * 4096);
    drop(permits);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn native_running_and_retained_credit_limits_are_independent() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let mut jobs = Vec::new();
    for id in 1..=384 {
        let boundary = Boundary::new(false, false);
        let job = permit(&rig, &scope, id)?.bind_input(Box::new(Input(boundary.clone())));
        boundary.started.wait()?;
        jobs.push((job, boundary));
    }
    let mut pending = Vec::new();
    for id in 385..=512 {
        pending.push(permit(&rig, &scope, id)?);
    }
    assert_eq!(scope.snapshot().running, 384);
    assert_eq!(scope.snapshot().queued, 128);
    assert!(matches!(
        reserve(&rig.client.core, &scope, native_spec(513)?, NativeCorrelation::new(513)?),
        Err(AdmissionError::CompletionSaturated)
    ));
    let mut ordinary =
        rig.client.submit_async(&rig.scope, spec(513, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, String>(())
        })?;
    assert!(matches!(rig.completion(&mut ordinary)?.outcome, JobOutcome::Success(())));
    let boundary = Boundary::new(false, false);
    let reserved = pending.pop().ok_or("Missing native reservation")?;
    let mut next = reserved.bind_input(Box::new(Input(boundary.clone())));
    assert_eq!(boundary.starts.load(Ordering::Acquire), 0);
    jobs[0].0.cancel();
    assert!(matches!(rig.completion(&mut jobs[0].0)?.outcome, JobOutcome::Cancelled));
    boundary.started.wait()?;
    next.cancel();
    assert!(matches!(rig.completion(&mut next)?.outcome, JobOutcome::Cancelled));
    drop(jobs);
    drop(pending);
    Ok(())
}

#[test]
fn native_scope_bound_cannot_exhaust_ordinary_scope_allowance() -> TestResult {
    let rig = Rig::new()?;
    let mut scopes = Vec::new();
    for id in 2..=133 {
        scopes.push(rig.client.core.create_scope_kind(
            ScopeSpec { id: ScopeId::new(id)?, deadline: None },
            Some(&rig.scope),
            true,
        )?);
    }
    assert!(matches!(
        rig.client.core.create_scope_kind(
            ScopeSpec { id: ScopeId::new(134)?, deadline: None },
            None,
            true
        ),
        Err(AdmissionError::ScopeCapacity)
    ));
    let ordinary =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(134)?, deadline: None }, None)?;
    assert!(!ordinary.snapshot().closed);
    assert_eq!(rig.scope.snapshot().children_live, 132);
    Ok(())
}
