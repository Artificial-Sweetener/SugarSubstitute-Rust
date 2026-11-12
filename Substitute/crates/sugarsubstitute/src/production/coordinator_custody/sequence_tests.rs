// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the actual shutdown owner with external protocol/family facts and real pipe retirement.
use super::*;
#[path = "sequence/boundary.rs"]
mod boundary;
use boundary::{Event, Fixture};
#[path = "sequence/profile.rs"]
mod profile;
/// Dependents that never settle cannot hold the helper past the final exchange bound,
/// one second before the original fifteen-second total.
#[test]
fn missing_profile_receipt_cannot_veto_helper_force_at_the_final_exchange_bound() -> TestResult {
    let mut fixture = Fixture::new(None)?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let mut owner = Retirement::default();
    owner.advance(budget, true, false, budget.started, key()?, &mut fixture.port)?;
    assert_eq!(fixture.port.events, vec![Event::Observe]);
    fixture.clock.set(Moment::from_nanos(13_999_000_000));
    owner.advance(budget, true, false, fixture.client.now()?, key()?, &mut fixture.port)?;
    assert_eq!(fixture.port.events, vec![Event::Observe, Event::Observe]);
    assert_eq!(
        owner.deadlines().into_iter().flatten().min(),
        Some(Moment::from_nanos(14_000_000_000))
    );
    fixture.clock.set(Moment::from_nanos(14_000_000_000));
    owner.advance(budget, true, false, fixture.client.now()?, key()?, &mut fixture.port)?;
    assert_eq!(
        fixture.port.events,
        vec![Event::Observe, Event::Observe, Event::Observe, Event::Stop, Event::Force]
    );
    assert_eq!(
        fixture.port.original,
        Some(ShutdownBudget { graceful_until: Moment::from_nanos(14_000_000_000), ..budget })
    );
    assert!(!owner.finish_received && !owner.settled && !owner.pipe_closed);
    Ok(())
}
/// A desktop family forced at grace that settles late still gets its Finish exchange,
/// where the former fixed twelve-second bound would already have forced the helper.
#[test]
fn slow_dependent_settlement_still_receives_finish_before_the_bound() -> TestResult {
    let mut fixture = Fixture::new(None)?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let mut owner = Retirement::default();
    for now in [budget.graceful_until, Moment::from_nanos(12_000_000_000)] {
        fixture.clock.set(now);
        owner.advance(budget, true, false, fixture.client.now()?, key()?, &mut fixture.port)?;
    }
    fixture.clock.set(Moment::from_nanos(12_500_000_000));
    owner.advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)?;
    fixture.clock.set(Moment::from_nanos(13_200_000_000));
    fixture.port.finished = true;
    owner.advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)?;
    assert_eq!(
        fixture.port.events,
        vec![Event::Observe, Event::Observe, Event::FinishPending, Event::Finished, Event::Stop]
    );
    assert!(owner.finish_received);
    assert_eq!(
        fixture.port.original,
        Some(ShutdownBudget { graceful_until: Moment::from_nanos(14_000_000_000), ..budget })
    );
    Ok(())
}
/// After settlement a helper that never answers Finish is forced at the exchange bound.
#[test]
fn unanswered_finish_is_forced_at_the_exchange_bound_after_settlement() -> TestResult {
    let mut fixture = Fixture::new(None)?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let mut owner = Retirement::default();
    fixture.clock.set(Moment::from_nanos(11_000_000_000));
    owner.advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)?;
    assert_eq!(
        owner.deadlines().into_iter().flatten().min(),
        Some(Moment::from_nanos(13_000_000_000))
    );
    fixture.clock.set(Moment::from_nanos(12_999_000_000));
    owner.advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)?;
    assert_eq!(fixture.port.events, vec![Event::FinishPending, Event::FinishPending]);
    fixture.clock.set(Moment::from_nanos(13_000_000_000));
    owner.advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)?;
    assert_eq!(
        fixture.port.events,
        vec![
            Event::FinishPending,
            Event::FinishPending,
            Event::FinishPending,
            Event::Stop,
            Event::Force
        ]
    );
    assert_eq!(
        fixture.port.original,
        Some(ShutdownBudget { graceful_until: Moment::from_nanos(13_000_000_000), ..budget })
    );
    assert!(!owner.finish_received && !owner.settled);
    Ok(())
}
#[test]
fn pending_finish_has_bounded_exchange_grace_within_original_total() -> TestResult {
    let mut fixture = Fixture::new(None)?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let mut owner = Retirement::default();
    for now in [budget.started, budget.graceful_until, Moment::from_nanos(12_000_000_000)] {
        fixture.clock.set(now);
        owner.advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)?;
        assert_eq!(owner.original, Some(budget));
        if now == budget.started {
            assert_eq!(
                owner.deadlines(),
                [
                    Some(budget.graceful_until),
                    Some(Moment::from_nanos(12_000_000_000)),
                    Some(budget.settle_until)
                ]
            );
        }
        assert!(!owner.finish_received && !owner.settled && owner.terminal.is_none());
    }
    assert_eq!(
        fixture.port.events,
        vec![
            Event::FinishPending,
            Event::FinishPending,
            Event::FinishPending,
            Event::Stop,
            Event::Force
        ]
    );
    assert_eq!(owner.deadlines().into_iter().flatten().min(), Some(budget.settle_until));
    Ok(())
}
#[test]
fn backend_force_waits_for_release_before_the_first_finish_exchange() -> TestResult {
    let mut fixture = Fixture::new(None)?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let mut owner = Retirement::default();
    fixture.clock.set(budget.graceful_until);
    owner.advance(budget, true, false, fixture.client.now()?, key()?, &mut fixture.port)?;
    assert_eq!(fixture.port.events, vec![Event::Observe]);
    assert_eq!(
        owner.deadlines().into_iter().flatten().min(),
        Some(Moment::from_nanos(14_000_000_000))
    );
    fixture.clock.set(Moment::from_nanos(10_100_000_000));
    fixture.port.finished = true;
    owner.advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)?;
    assert_eq!(fixture.port.events, vec![Event::Observe, Event::Finished, Event::Stop]);
    assert!(owner.finish_received);
    assert_eq!(
        fixture.port.original,
        Some(ShutdownBudget { graceful_until: Moment::from_nanos(12_100_000_000), ..budget })
    );
    Ok(())
}
#[test]
fn late_finished_is_observed_after_helper_force_without_renewing_cleanup() -> TestResult {
    let mut fixture = Fixture::new(Some(81))?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let key = fixture.port.key()?;
    let mut owner = Retirement::default();
    owner.advance(budget, true, true, fixture.client.now()?, key, &mut fixture.port)?;
    fixture.clock.set(Moment::from_nanos(12_000_000_000));
    owner.advance(budget, true, true, fixture.client.now()?, key, &mut fixture.port)?;
    fixture.clock.set(Moment::from_nanos(13_000_000_000));
    fixture.port.finished = true;
    owner.advance(budget, true, true, fixture.client.now()?, key, &mut fixture.port)?;
    assert_eq!(
        fixture.port.events,
        vec![
            Event::FinishPending,
            Event::FinishPending,
            Event::Stop,
            Event::Force,
            Event::Finished,
            Event::Force
        ]
    );
    assert!(!owner.settled && !owner.pipe_closed);
    fixture.port.family_retired = true;
    fixture.settle(&mut owner, budget, true, key)?;
    assert!(owner.finish_received && owner.settled && owner.pipe_closed);
    assert!(matches!(
        owner.terminal.as_ref().ok_or("Actual terminal absent")?.outcome,
        JobOutcome::Cancelled
    ));
    assert_eq!(
        fixture.port.original,
        Some(ShutdownBudget { graceful_until: Moment::from_nanos(12_000_000_000), ..budget })
    );
    assert_eq!(fixture.port.events.iter().filter(|event| **event == Event::Finished).count(), 1);
    drop(owner);
    fixture.close_fixture()?;
    Ok(())
}
#[test]
fn original_protocol_failure_survives_actual_cleanup_error() -> TestResult {
    let mut fixture = Fixture::new(None)?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let native = sugarsubstitute_execution::windows::NativeObservationError::new(
        "original_finish",
        std::io::Error::from_raw_os_error(109),
    );
    let pointer = std::ptr::from_ref(native.source().ok_or("Original source absent")?);
    fixture.port.failure = Some(native);
    fixture.port.cleanup_failure = true;
    let mut owner = Retirement::default();
    for _ in 0..2 {
        let cause = owner
            .advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)
            .err()
            .ok_or("Failed cleanup accepted")?;
        let original = cause
            .source()
            .and_then(|value| {
                value.downcast_ref::<sugarsubstitute_execution::windows::NativeObservationError>()
            })
            .ok_or("Original cause replaced")?;
        assert_eq!(std::ptr::from_ref(original.source().ok_or("Retained source absent")?), pointer);
    }
    assert_eq!(
        fixture.port.original,
        Some(ShutdownBudget { graceful_until: Moment::from_nanos(12_000_000_000), ..budget })
    );
    assert!(!owner.settled && !owner.finish_received && !owner.pipe_closed);
    Ok(())
}
#[test]
fn original_cutoff_without_finished_preserves_failed_owned_cleanup() -> TestResult {
    let mut fixture = Fixture::new(None)?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let mut owner = Retirement::default();
    owner.advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)?;
    fixture.clock.set(budget.settle_until);
    let cause = owner
        .advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)
        .err()
        .ok_or("Missing Finished accepted")?;
    assert!(cause.source().and_then(|value|value.downcast_ref::<RetirementRejected>()).is_some_and(|value|matches!(value,RetirementRejected::FinishExpired { until, dependents_retired:true, status:BackendCustodyFinishStatus::Unissued } if *until==budget.settle_until)));
    assert_eq!(fixture.port.events, vec![Event::FinishPending, Event::Stop, Event::ExpiredCleanup]);
    assert_eq!(
        fixture.port.original,
        Some(ShutdownBudget { graceful_until: Moment::from_nanos(12_000_000_000), ..budget })
    );
    assert!(!owner.settled && !owner.finish_received && owner.terminal.is_none());
    assert_eq!(owner.deadlines().into_iter().flatten().min(), None);
    Ok(())
}
#[test]
fn changed_original_budget_rejects_before_another_external_effect() -> TestResult {
    let mut fixture = Fixture::new(None)?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let mut owner = Retirement::default();
    owner.advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)?;
    let before = fixture.port.events.clone();
    let changed = ShutdownBudget::new(Moment::from_nanos(1))?;
    assert!(
        owner
            .advance(changed, true, true, fixture.client.now()?, key()?, &mut fixture.port)
            .is_err()
    );
    assert_eq!(owner.original, Some(budget));
    assert_eq!(fixture.port.events, before);
    Ok(())
}

#[test]
fn original_finish_cutoff_distinguishes_profile_fence_unissued_and_queued_nonce() -> TestResult {
    for (dependents_retired, status, text) in [
        (false, BackendCustodyFinishStatus::Unissued, "backend or profile retirement pending"),
        (true, BackendCustodyFinishStatus::Unissued, "Finish not issued before cutoff"),
        (true, BackendCustodyFinishStatus::Issued(7), "Finish nonce 7 queued"),
        (false, BackendCustodyFinishStatus::Unavailable, "Finish state unavailable"),
    ] {
        let mut fixture = Fixture::new(None)?;
        fixture.port.finish_status = status;
        let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
        let mut owner = Retirement::default();
        owner.advance(
            budget,
            true,
            dependents_retired,
            budget.started,
            key()?,
            &mut fixture.port,
        )?;
        fixture.clock.set(budget.settle_until);
        let cause = owner
            .advance(
                budget,
                true,
                dependents_retired,
                budget.settle_until,
                key()?,
                &mut fixture.port,
            )
            .err()
            .ok_or("Cutoff admitted a missing Finished")?;
        let original = cause
            .source()
            .and_then(|source| source.downcast_ref::<RetirementRejected>())
            .ok_or("Original cutoff cause replaced")?;
        assert!(matches!(original, RetirementRejected::FinishExpired {
            until, dependents_retired: actual_profiles, status: actual_status,
        } if *until == budget.settle_until && *actual_profiles == dependents_retired && *actual_status == status));
        assert!(original.to_string().contains(text));
        assert!(!owner.finish_received && !owner.pipe_closed && !owner.settled);
        assert_eq!(owner.original, Some(budget));
    }
    Ok(())
}
#[test]
fn unestablished_startup_cleanup_requires_actual_pipe_retirement() -> TestResult {
    let mut fixture = Fixture::new(Some(82))?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let key = fixture.port.key()?;
    let mut owner = Retirement::default();
    fixture.port.family_retired = true;
    fixture.settle(&mut owner, budget, false, key)?;
    assert!(owner.settled && !owner.finish_received && owner.pipe_closed);
    assert!(
        !fixture
            .port
            .events
            .iter()
            .any(|event| matches!(event, Event::Finished | Event::FinishPending))
    );
    drop(owner);
    fixture.close_fixture()?;
    Ok(())
}

/// Observing an actually stopped pipe preserves its terminal without claiming family retirement.
#[test]
fn stopped_pipe_terminal_is_adopted_before_fresh_admission_and_retained_until_cleanup() -> TestResult
{
    use sugarsubstitute_contracts::execution::JobId;
    let mut fixture = Fixture::new(Some(111))?;
    let key = fixture.port.key()?;
    fixture.port.parent()?.close()?;
    fixture.terminal(JobId::new(1)?)?;
    let mut owner = Retirement::default();
    for _ in 0..2 {
        let cause = fixture
            .port
            .observe_terminal(&mut owner)
            .err()
            .ok_or("Stopped pipe granted fresh admission")?;
        assert!(matches!(
            cause.downcast_ref::<RetirementRejected>(),
            Some(RetirementRejected::Terminal(TerminalKind::Cancelled))
        ));
        let terminal = owner.terminal.as_ref().ok_or("Actual pipe terminal lost")?;
        assert_eq!(terminal.spec.id, JobId::new(1)?);
        assert_eq!(terminal.scope_id, fixture.scope.id());
        assert!(matches!(terminal.outcome, JobOutcome::Cancelled));
        assert!(!owner.settled && !owner.pipe_closed && owner.original.is_none());
        assert!(fixture.scope.snapshot().reserved_bytes > 0);
    }
    let budget = ShutdownBudget::new(fixture.client.now()?)?;
    fixture.port.family_retired = true;
    fixture.settle(&mut owner, budget, false, key)?;
    assert!(owner.settled && owner.pipe_closed);
    assert_eq!(owner.original, Some(budget));
    assert!(fixture.scope.snapshot().reserved_bytes > 0);
    drop(owner);
    fixture.close_fixture()?;
    Ok(())
}

#[test]
fn late_pipe_completion_is_consumed_even_when_family_cleanup_fails() -> TestResult {
    use sugarsubstitute_contracts::execution::JobId;
    let mut fixture = Fixture::new(Some(113))?;
    let key = fixture.port.key()?;
    let budget = ShutdownBudget::new(fixture.client.now()?)?;
    let mut owner = Retirement::default();
    fixture.port.cleanup_failure = true;
    assert!(owner.advance(budget, false, false, budget.started, key, &mut fixture.port).is_err());
    fixture.port.parent()?.close()?;
    fixture.terminal(JobId::new(1)?)?;
    assert!(owner.advance(budget, false, false, budget.started, key, &mut fixture.port).is_err());
    assert!(owner.terminal.is_some());
    assert!(!owner.settled && !owner.pipe_closed);
    assert!(fixture.port.events.contains(&Event::Terminal));
    drop(owner);
    fixture.close_fixture()?;
    Ok(())
}
