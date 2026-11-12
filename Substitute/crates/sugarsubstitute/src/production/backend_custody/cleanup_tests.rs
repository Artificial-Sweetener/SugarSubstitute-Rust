// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve real producer facts through pending observation and failed native encoding.
use super::*;
type Result = std::result::Result<(), Box<dyn Error>>;
fn counter_failure(stage: &'static str) -> NativeObservationError {
    NativeObservationError::new(stage, std::io::Error::other("original capture observation"))
}
#[test]
fn helper_pending_capture_cannot_create_shutdown_cutoff() -> Result {
    let mut cleanup = Cleanup::default();
    cleanup.observe(CaptureCleanupSnapshot::Requested)?;
    cleanup.observe(CaptureCleanupSnapshot::Requested)?;
    assert!(cleanup.until().is_none());
    assert!(cleanup.observe(CaptureCleanupSnapshot::Dormant).is_err());
    assert!(cleanup.until().is_none());
    Ok(())
}
#[test]
fn helper_counter_failure_retains_first_complete_budget_without_renewal() -> Result {
    let mut cleanup = Cleanup::default();
    cleanup.observe(CaptureCleanupSnapshot::Requested)?;
    let first = ShutdownBudget::new(Moment::from_nanos(50))?;
    let failure = cleanup
        .observe(CaptureCleanupSnapshot::Latched {
            budget: first,
            counter: Err(counter_failure("helper_original_counter")),
        })
        .err()
        .ok_or("Counter cause missing")?;
    assert_eq!(
        failure.downcast_ref::<NativeObservationError>().ok_or("Native cause type lost")?.stage(),
        "helper_original_counter"
    );
    assert_eq!(cleanup.budget, Some(first));
    assert_eq!(cleanup.until(), Some(first.settle_until));
    let later = ShutdownBudget::new(Moment::from_nanos(100_000_000_000))?;
    let repeated = cleanup
        .observe(CaptureCleanupSnapshot::Latched {
            budget: later,
            counter: Err(counter_failure("replacement_counter")),
        })
        .err()
        .ok_or("Original counter cause lost")?;
    assert_eq!(
        repeated.downcast_ref::<NativeObservationError>().ok_or("Native cause type lost")?.stage(),
        "helper_original_counter"
    );
    assert_eq!(cleanup.budget, Some(first));
    Ok(())
}
#[test]
fn helper_failed_first_clock_never_adopts_a_replacement_transaction() -> Result {
    let mut cleanup = Cleanup::default();
    assert!(
        cleanup
            .observe(CaptureCleanupSnapshot::ClockUnavailable(counter_failure(
                "helper_first_clock"
            )))
            .is_err()
    );
    let later = ShutdownBudget::new(Moment::from_nanos(100))?;
    let failure = cleanup
        .observe(CaptureCleanupSnapshot::Latched {
            budget: later,
            counter: Err(counter_failure("replacement_counter")),
        })
        .err()
        .ok_or("Original clock cause lost")?;
    assert_eq!(
        failure.downcast_ref::<NativeObservationError>().ok_or("Native cause type lost")?.stage(),
        "helper_first_clock"
    );
    assert!(cleanup.until().is_none());
    Ok(())
}
