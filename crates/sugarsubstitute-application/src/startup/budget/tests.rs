// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify original campaign boundaries without clocks, scheduling or external I/O.

use super::*;

#[test]
fn connection_kind_preserves_distinct_original_campaigns() -> Result<(), TimeError> {
    let started = Moment::from_nanos(17);
    let attached = ConnectionBudget::new(ConnectionBudgetKind::Attached, started)?;
    let managed = ConnectionBudget::new(ConnectionBudgetKind::Managed, started)?;
    assert_eq!(attached.kind(), ConnectionBudgetKind::Attached);
    assert_eq!(managed.kind(), ConnectionBudgetKind::Managed);
    assert_eq!(attached.started(), started);
    assert_eq!(managed.started(), started);
    assert_eq!(attached.until().as_nanos(), 30_000_000_017);
    assert_eq!(managed.until().as_nanos(), 180_000_000_017);
    Ok(())
}

#[test]
fn retries_use_two_seconds_without_renewing_original_deadline() -> Result<(), Box<dyn Error>> {
    let budget = ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?;
    let original = budget;
    assert_eq!(budget.probe_until(budget.started())?.as_nanos(), 2_000_000_000);
    assert_eq!(budget.probe_until(Moment::from_nanos(25_000_000_000))?.as_nanos(), 27_000_000_000);
    assert_eq!(budget.probe_until(Moment::from_nanos(29_000_000_000))?, budget.until());
    assert_eq!(budget, original);
    Ok(())
}

#[test]
fn final_nanosecond_is_admitted_but_expiry_and_later_starts_are_rejected()
-> Result<(), Box<dyn Error>> {
    let budget = ConnectionBudget::new(ConnectionBudgetKind::Managed, Moment::from_nanos(0))?;
    let original = budget;
    assert_eq!(budget.probe_until(Moment::from_nanos(179_999_999_999))?, budget.until());
    assert_eq!(budget.probe_until(budget.until()), Err(ProbeBudgetError::Expired));
    assert_eq!(
        budget.probe_until(Moment::from_nanos(180_000_000_001)),
        Err(ProbeBudgetError::Expired)
    );
    assert_eq!(budget, original);
    Ok(())
}

#[test]
fn earlier_clock_start_is_rejected_without_changing_campaign() -> Result<(), Box<dyn Error>> {
    let budget = ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(100))?;
    let original = budget;
    assert_eq!(
        budget.probe_until(Moment::from_nanos(99)),
        Err(ProbeBudgetError::Clock(TimeError::Regression))
    );
    assert_eq!(budget.probe_until(budget.started())?.as_nanos(), 2_000_000_100);
    assert_eq!(budget, original);
    Ok(())
}

#[test]
fn connection_deadline_overflow_rejects_both_campaign_kinds() -> Result<(), TimeError> {
    for (kind, duration_nanos) in [
        (ConnectionBudgetKind::Attached, 30_000_000_000),
        (ConnectionBudgetKind::Managed, 180_000_000_000),
    ] {
        let last_start = Moment::from_nanos(u64::MAX - duration_nanos);
        assert_eq!(ConnectionBudget::new(kind, last_start)?.until(), Moment::from_nanos(u64::MAX));
        assert_eq!(
            ConnectionBudget::new(kind, Moment::from_nanos(last_start.as_nanos() + 1)),
            Err(TimeError::Overflow)
        );
    }
    Ok(())
}

#[test]
fn near_clock_limit_probe_caps_before_arithmetic() -> Result<(), Box<dyn Error>> {
    let budget = ConnectionBudget::new(
        ConnectionBudgetKind::Attached,
        Moment::from_nanos(u64::MAX - 30_000_000_000),
    )?;
    assert_eq!(budget.probe_until(Moment::from_nanos(u64::MAX - 1))?, budget.until());
    assert_eq!(budget.probe_until(Moment::from_nanos(u64::MAX)), Err(ProbeBudgetError::Expired));
    Ok(())
}

#[test]
fn probe_clock_rejection_preserves_typed_source_but_expiry_has_no_clock_cause()
-> Result<(), TimeError> {
    let budget = ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(1))?;
    let Err(error) = budget.probe_until(Moment::from_nanos(0)) else {
        return Err(TimeError::Regression);
    };
    assert_eq!(
        error.source().and_then(|cause| cause.downcast_ref::<TimeError>()),
        Some(&TimeError::Regression)
    );
    assert!(ProbeBudgetError::Expired.source().is_none());
    Ok(())
}

#[test]
fn frontend_grants_retain_original_thirty_second_budget() -> Result<(), TimeError> {
    let started = Moment::from_nanos(41);
    let first_grant = FrontendBudget::new(started)?;
    let repeated_grant = first_grant;
    assert_eq!(first_grant.started(), started);
    assert_eq!(first_grant.until().as_nanos(), 30_000_000_041);
    assert_eq!(repeated_grant.started(), first_grant.started());
    assert_eq!(repeated_grant.until(), first_grant.until());
    Ok(())
}

#[test]
fn frontend_deadline_admits_last_start_and_rejects_overflow() -> Result<(), TimeError> {
    let last_start = Moment::from_nanos(u64::MAX - 30_000_000_000);
    assert_eq!(FrontendBudget::new(last_start)?.until(), Moment::from_nanos(u64::MAX));
    assert_eq!(
        FrontendBudget::new(Moment::from_nanos(last_start.as_nanos() + 1)),
        Err(TimeError::Overflow)
    );
    Ok(())
}
