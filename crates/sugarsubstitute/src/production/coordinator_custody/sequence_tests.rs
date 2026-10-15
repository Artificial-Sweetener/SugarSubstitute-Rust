// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the actual shutdown owner with external protocol/family facts and real pipe retirement.
use super::*;
#[path = "sequence/boundary.rs"]
mod boundary;
use boundary::{Event, Fixture};
#[path = "sequence/profile.rs"]
mod profile;
#[test]
fn original_pending_release_or_finish_keeps_helper_stop_and_force_unissued() -> TestResult {
    let mut fixture = Fixture::new(None)?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let mut owner = Retirement::default();
    for now in [budget.started, budget.graceful_until, Moment::from_nanos(12_000_000_000)] {
        fixture.clock.set(now);
        owner.advance(budget, true, true, fixture.client.now()?, key()?, &mut fixture.port)?;
        assert_eq!(owner.original, Some(budget));
        assert!(!owner.finish_received && !owner.settled && owner.terminal.is_none());
    }
    assert!(fixture.port.events.iter().all(|event| matches!(event, Event::FinishPending)));
    assert_eq!(owner.next_deadline(), Some(budget.settle_until));
    Ok(())
}
#[test]
fn late_finished_precedes_helper_force_and_actual_pipe_terminal() -> TestResult {
    let mut fixture = Fixture::new(Some(81))?;
    let budget = ShutdownBudget::new(Moment::from_nanos(0))?;
    let key = fixture.port.key()?;
    let mut owner = Retirement::default();
    fixture.clock.set(budget.graceful_until);
    owner.advance(budget, true, true, fixture.client.now()?, key, &mut fixture.port)?;
    fixture.clock.set(Moment::from_nanos(12_000_000_000));
    fixture.port.finished = true;
    owner.advance(budget, true, true, fixture.client.now()?, key, &mut fixture.port)?;
    assert_eq!(
        fixture.port.events,
        vec![Event::FinishPending, Event::Finished, Event::Stop, Event::Force]
    );
    assert!(!owner.settled && !owner.pipe_closed);
    fixture.port.family_retired = true;
    fixture.settle(&mut owner, budget, true, key)?;
    assert!(owner.finish_received && owner.settled && owner.pipe_closed);
    assert!(matches!(
        owner.terminal.as_ref().ok_or("Actual terminal absent")?.outcome,
        JobOutcome::Cancelled
    ));
    assert_eq!(fixture.port.original, Some(budget));
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
    assert_eq!(fixture.port.original, Some(budget));
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
    assert!(cause.source().and_then(|value|value.downcast_ref::<RetirementRejected>()).is_some_and(|value|matches!(value,RetirementRejected::FinishExpired(until) if *until==budget.settle_until)));
    assert_eq!(fixture.port.events, vec![Event::FinishPending, Event::Stop, Event::ExpiredCleanup]);
    assert_eq!(fixture.port.original, Some(budget));
    assert!(!owner.settled && !owner.finish_received && owner.terminal.is_none());
    assert_eq!(owner.next_deadline(), None);
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
