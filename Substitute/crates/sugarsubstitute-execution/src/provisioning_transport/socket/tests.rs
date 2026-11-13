// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Trigger controls inside the same poll after their original supervisor has registered them.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobClass, JobOutcome, Moment, ScopeId, ScopeSpec,
};

/// A socket stop cannot complete successfully ahead of its original cancellation supervisor.
#[test]
fn same_poll_stop_keeps_cancellation_classification_with_the_supervisor() -> TestResult {
    let rig = Rig::new()?;
    let scope = Arc::new(
        rig.client
            .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(&rig.scope))?,
    );
    let cancelled = scope.clone();
    let budget = memory::Budget::new();
    let mut job = rig.client.submit_async(
        &scope,
        spec(1, JobClass::IoOrdinary, RESERVATION_BYTES)?,
        move |context| async move {
            budget.activate(context.memory())?;
            cancelled.cancel();
            stop(&context, &budget).await
        },
    )?;
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled), "{:?}", completion.outcome);
    drop(completion);
    drop(job);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Exhausted delivery admission cannot turn cancellation/deadline controls into retained I/O errors.
#[test]
fn same_poll_full_delivery_keeps_control_classification_with_the_supervisor() -> TestResult {
    for timeout in [false, true] {
        let rig = Rig::new()?;
        let scope =
            Arc::new(rig.client.create_scope(
                ScopeSpec { id: ScopeId::new(2)?, deadline: None },
                Some(&rig.scope),
            )?);
        let cancelled = scope.clone();
        let clock = rig.clock.clone();
        let (transport, mut runner) = Transport::pair();
        let mut request = spec(1, JobClass::IoOrdinary, RESERVATION_BYTES)?;
        if timeout {
            request.deadline = Some(Moment::from_nanos(1));
        }
        let mut job = rig.client.submit_async(&scope, request, move |context| async move {
            runner.budget.activate(context.memory())?;
            let mut held = Vec::new();
            for _ in 0..QUEUE_SLOTS {
                held.push(runner.budget.permit()?);
            }
            if timeout {
                clock.advance(1);
            } else {
                cancelled.cancel();
            }
            runner.deliver(&context, "{\"event\":\"progress\"}".into()).await
        })?;
        let completion = rig.completion(&mut job)?;
        assert!(
            matches!(
                (&completion.outcome, timeout),
                (JobOutcome::Cancelled, false) | (JobOutcome::TimedOut, true)
            ),
            "{:?}",
            completion.outcome
        );
        drop(completion);
        drop(transport);
        drop(job);
        drop(rig.owner);
        assert_eq!(scope.snapshot().reserved_bytes, 0);
    }
    Ok(())
}

/// Endpoint retirement in the same poll cannot override an already requested terminal control.
#[test]
fn simultaneous_endpoint_closure_does_not_override_original_controls() -> TestResult {
    for timeout in [false, true] {
        let rig = Rig::new()?;
        let scope =
            Arc::new(rig.client.create_scope(
                ScopeSpec { id: ScopeId::new(2)?, deadline: None },
                Some(&rig.scope),
            )?);
        let cancelled = scope.clone();
        let clock = rig.clock.clone();
        let budget = memory::Budget::new();
        let mut request = spec(1, JobClass::IoOrdinary, RESERVATION_BYTES)?;
        if timeout {
            request.deadline = Some(Moment::from_nanos(1));
        }
        let mut job = rig.client.submit_async(&scope, request, move |context| async move {
            budget.activate(context.memory())?;
            if timeout {
                clock.advance(1);
            } else {
                cancelled.cancel();
            }
            budget.close();
            stop(&context, &budget).await
        })?;
        let completion = rig.completion(&mut job)?;
        assert!(
            matches!(
                (&completion.outcome, timeout),
                (JobOutcome::Cancelled, false) | (JobOutcome::TimedOut, true)
            ),
            "{:?}",
            completion.outcome
        );
        drop(completion);
        drop(job);
        drop(rig.owner);
        assert_eq!(scope.snapshot().reserved_bytes, 0);
    }
    Ok(())
}

/// Free delivery capacity does not permit a stale frame after cancellation or deadline expiry.
#[test]
fn available_delivery_capacity_cannot_publish_after_original_controls() -> TestResult {
    for timeout in [false, true] {
        let rig = Rig::new()?;
        let scope =
            Arc::new(rig.client.create_scope(
                ScopeSpec { id: ScopeId::new(2)?, deadline: None },
                Some(&rig.scope),
            )?);
        let cancelled = scope.clone();
        let clock = rig.clock.clone();
        let (transport, mut runner) = Transport::pair();
        let mut request = spec(1, JobClass::IoOrdinary, RESERVATION_BYTES)?;
        if timeout {
            request.deadline = Some(Moment::from_nanos(1));
        }
        let mut job = rig.client.submit_async(&scope, request, move |context| async move {
            runner.budget.activate(context.memory())?;
            if timeout {
                clock.advance(1);
            } else {
                cancelled.cancel();
            }
            runner.deliver(&context, "{\"event\":\"progress\"}".into()).await
        })?;
        let completion = rig.completion(&mut job)?;
        assert!(
            matches!(
                (&completion.outcome, timeout),
                (JobOutcome::Cancelled, false) | (JobOutcome::TimedOut, true)
            ),
            "{:?}",
            completion.outcome
        );
        assert!(transport.next_event().is_none());
        drop(completion);
        drop(transport);
        drop(job);
        drop(rig.owner);
        assert_eq!(scope.snapshot().reserved_bytes, 0);
    }
    Ok(())
}
