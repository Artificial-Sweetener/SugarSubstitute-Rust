// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Bound combined completion metadata while preserving separate native credit and wake interests.

use super::*;
use std::{num::NonZeroUsize, time::Duration};

pub(super) fn wait_terminal(rig: &Rig, id: u64) -> TestResult {
    let id = JobId::new(id)?;
    let (state, timeout) = rig
        .client
        .core
        .cpu_changed
        .wait_timeout_while(lock(&rig.client.core.state), Duration::from_secs(10), |state| {
            state.jobs.get(&id).is_some_and(|record| !matches!(record.phase, JobPhase::Terminal(_)))
        })
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    if timeout.timed_out()
        && state.jobs.get(&id).is_some_and(|record| !matches!(record.phase, JobPhase::Terminal(_)))
    {
        return Err("Owned terminal transaction was not observed".into());
    }
    Ok(())
}

#[test]
fn native_and_ordinary_flood_is_bounded_at560_notices_and_drains32_without_ack() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let mut native_jobs = Vec::new();
    for id in 1..=512 {
        let boundary = Boundary::new(false, false);
        let job = permit(&rig, &scope, id)?.bind_input(Box::new(Input(boundary.clone())));
        boundary.started.wait()?;
        boundary.complete()?;
        wait_terminal(&rig, id)?;
        native_jobs.push(job);
    }
    let mut ordinary_jobs = Vec::new();
    for id in 513..=560 {
        let job = rig.client.submit_async(
            &rig.scope,
            spec(id, JobClass::IoLifecycle, 0)?,
            |_| async { Ok::<_, String>(()) },
        )?;
        wait_terminal(&rig, id)?;
        ordinary_jobs.push(job);
    }
    assert_eq!(scope.snapshot().terminal_unobserved, 512);
    assert_eq!(rig.scope.snapshot().terminal_unobserved, 48);
    assert!(matches!(
        reserve(&rig.client.core, &scope, native_spec(561)?, NativeCorrelation::new(561)?),
        Err(AdmissionError::CompletionSaturated)
    ));
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(561, JobClass::IoLifecycle, 0)?, |_| async {
            Ok::<_, String>(())
        }),
        Err(AdmissionError::CompletionSaturated)
    ));
    let first = rig.client.drain_completions(NonZeroUsize::MAX)?;
    assert_eq!(first.notifications.len(), 32);
    assert_eq!(first.remaining, 528);
    assert_eq!(scope.snapshot().terminal_unobserved, 512);
    let mut observed = first.notifications.len();
    while observed < 560 {
        observed += rig.client.drain_completions(NonZeroUsize::MAX)?.notifications.len();
    }
    assert_eq!(observed, 560);
    assert_eq!(lock(&rig.client.core.state).jobs.len(), 560);
    drop(native_jobs);
    drop(ordinary_jobs);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn native_ready_capacity_release_wakes_retry_without_terminal_fabrication() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let mut permits = Vec::new();
    for id in 1..=384 {
        permits.push(permit(&rig, &scope, id)?);
    }
    assert!(matches!(
        reserve(&rig.client.core, &scope, native_spec(385)?, NativeCorrelation::new(385)?),
        Err(AdmissionError::QueueSaturated { .. })
    ));
    let observed = rig.wake.count();
    drop(permits.pop());
    rig.wake.wait_after(observed)?;
    assert!(rig.client.drain_completions(NonZeroUsize::MAX)?.notifications.is_empty());
    permits.push(permit(&rig, &scope, 385)?);
    drop(permits);
    Ok(())
}

#[test]
fn native_peak_bytes_and_retained_result_share_the_ordinary_memory_limit() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let mut request = native_spec(1)?;
    request.reservation_bytes = 256 * 1024 * 1024;
    let boundary = Boundary::new(false, false);
    let mut job = reserve(&rig.client.core, &scope, request, NativeCorrelation::new(1)?)?
        .bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    boundary.complete()?;
    let completion = rig.completion(&mut job)?;
    assert!(matches!(
        rig.client.submit_async(&rig.scope, spec(2, JobClass::IoLifecycle, 1)?, |_| async {
            Ok::<_, String>(())
        }),
        Err(AdmissionError::MemorySaturated { available_bytes: 0, .. })
    ));
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 256 * 1024 * 1024);
    drop(completion);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
