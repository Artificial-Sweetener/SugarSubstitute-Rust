// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Make owned controls ready during the poll already entered by the real async supervisor.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobClass, JobOutcome, Moment, ScopeId, ScopeSpec,
};

/// A transport stop must not replace its original supervisor's cancellation or deadline outcome.
#[test]
fn same_poll_stop_preserves_the_supervisors_terminal_reason() -> TestResult {
    for (expired, closed) in [(false, false), (true, false), (false, true), (true, true)] {
        let rig = Rig::new()?;
        let scope =
            Arc::new(rig.client.create_scope(
                ScopeSpec { id: ScopeId::new(2)?, deadline: None },
                Some(&rig.scope),
            )?);
        let control = scope.clone();
        let clock = rig.clock.clone();
        let budget = memory::Budget::new();
        let mut request = spec(1, JobClass::IoOrdinary, RESERVATION_BYTES)?;
        if expired {
            request.deadline = Some(Moment::from_nanos(1));
        }
        let mut job = rig.client.submit_async(&scope, request, move |context| async move {
            budget.activate(context.memory())?;
            if expired {
                clock.advance(1);
            } else {
                control.cancel();
            }
            if closed {
                budget.close();
            }
            stop(&context, &budget).await
        })?;
        let completion = rig.completion(&mut job)?;
        assert!(
            matches!(
                (&completion.outcome, expired),
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

/// Available capacity and both backpressure boundaries preserve original controls and memory credit.
#[test]
fn same_poll_backpressure_preserves_controls_and_original_credit() -> TestResult {
    for (publication, available) in [(false, false), (true, false), (false, true)] {
        for expired in [false, true] {
            let rig = Rig::new()?;
            let scope = Arc::new(rig.client.create_scope(
                ScopeSpec { id: ScopeId::new(2)?, deadline: None },
                Some(&rig.scope),
            )?);
            let control = scope.clone();
            let clock = rig.clock.clone();
            let (transport, mut runner) = Transport::pair();
            let mut request = spec(1, JobClass::IoOrdinary, RESERVATION_BYTES)?;
            if expired {
                request.deadline = Some(Moment::from_nanos(1));
            }
            let mut job = rig.client.submit_async(&scope, request, move |context| async move {
                runner.budget.activate(context.memory())?;
                let mut retained = Vec::new();
                let pending = if publication {
                    runner
                        .events
                        .try_send(TransportEvent::Listening { port: 1, token: "fixture".into() })
                        .map_err(io::Error::other)?;
                    for _ in 1..QUEUE_SLOTS {
                        runner
                            .events
                            .try_send(TransportEvent::Frame(Frame {
                                bytes: "{}".into(),
                                _permit: runner.budget.permit()?,
                            }))
                            .map_err(io::Error::other)?;
                    }
                    Some(TransportEvent::Frame(Frame {
                        bytes: "{}".into(),
                        _permit: runner.budget.permit()?,
                    }))
                } else {
                    if !available {
                        for _ in 0..QUEUE_SLOTS {
                            retained.push(runner.budget.permit()?);
                        }
                    }
                    None
                };
                if expired {
                    clock.advance(1);
                } else {
                    control.cancel();
                }
                match pending {
                    Some(event) => runner.publish(&context, event).await,
                    None => runner.deliver(&context, "{\"event\":\"progress\"}".into()).await,
                }
            })?;
            let completion = rig.completion(&mut job)?;
            assert!(
                matches!(
                    (&completion.outcome, expired),
                    (JobOutcome::Cancelled, false) | (JobOutcome::TimedOut, true)
                ),
                "{:?}",
                completion.outcome
            );
            drop(completion);
            drop(job);
            if available {
                assert!(transport.next_event().is_none());
            }
            drop(transport);
            drop(rig.owner);
            assert_eq!(scope.snapshot().reserved_bytes, 0);
        }
    }
    Ok(())
}
