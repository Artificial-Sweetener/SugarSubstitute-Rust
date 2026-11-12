// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Verify application admission against real bounded receipt parsing.

use super::*;
use serde_json::json;

/// Construct complete wire evidence at the real protocol boundary.
fn receipt(launch: u64, navigation: u64, failed: bool) -> Result<ReadinessReceipt, Box<dyn Error>> {
    let outcome = if failed {
        json!({"status": "failed", "failure": {"code": "timed_out", "detail": "probe expired"}})
    } else {
        json!({"status": "ready", "evidence": {
            "frontend_version": "1.47.11", "sugarcubes_version": "0.15.0",
            "app_graph_ready": true, "canvas_connected": true, "loading_finished": true,
            "sugarcubes_registered": true, "sugarcubes_ready": true
        }})
    };
    let bytes = serde_json::to_vec(&json!({
        "schema_version": 1, "launch_id": launch,
        "navigation_generation": navigation, "outcome": outcome
    }))?;
    Ok(ReadinessReceipt::parse(&bytes)?)
}

/// A valid completion consumes the current navigation's terminal allowance once.
#[test]
fn accepts_one_current_completion_and_rejects_duplicates() -> Result<(), Box<dyn Error>> {
    let mut gate = ReadinessGate::new(LaunchId::new(7)?, Generation::new(3)?);
    assert_eq!(gate.launch().get(), 7);
    assert_eq!(gate.navigation().get(), 3);
    assert!(matches!(
        gate.accept(receipt(7, 3, false)?),
        ReadinessDecision::Accepted(ReadinessOutcome::Ready { .. })
    ));
    assert_eq!(
        gate.accept(receipt(7, 3, false)?),
        ReadinessDecision::Rejected(ReceiptRejection::AlreadyCompleted)
    );
    assert_eq!(
        gate.accept(receipt(7, 3, true)?),
        ReadinessDecision::Rejected(ReceiptRejection::AlreadyCompleted)
    );
    Ok(())
}

/// Delayed and unrequested callbacks cannot consume the valid current result.
#[test]
fn rejects_foreign_launch_and_past_or_future_navigation_without_consuming()
-> Result<(), Box<dyn Error>> {
    let mut gate = ReadinessGate::new(LaunchId::new(7)?, Generation::new(3)?);
    assert_eq!(
        gate.accept(receipt(8, 3, false)?),
        ReadinessDecision::Rejected(ReceiptRejection::ForeignLaunch)
    );
    for navigation in [2, 4] {
        assert_eq!(
            gate.accept(receipt(7, navigation, false)?),
            ReadinessDecision::Rejected(ReceiptRejection::WrongNavigation)
        );
    }
    assert!(matches!(gate.accept(receipt(7, 3, false)?), ReadinessDecision::Accepted(_)));
    Ok(())
}

/// Failure is terminal and its original detail survives transfer to startup policy.
#[test]
fn failure_consumes_terminal_allowance_and_retains_original_detail() -> Result<(), Box<dyn Error>> {
    let mut gate = ReadinessGate::new(LaunchId::new(7)?, Generation::new(3)?);
    assert!(matches!(gate.accept(receipt(7, 3, true)?), ReadinessDecision::Accepted(
        ReadinessOutcome::Failed { failure }
    ) if failure.detail == "probe expired"));
    assert_eq!(
        gate.accept(receipt(7, 3, false)?),
        ReadinessDecision::Rejected(ReceiptRejection::AlreadyCompleted)
    );
    Ok(())
}

/// Reload invalidates old callbacks and opens one allowance for its newer epoch.
#[test]
fn navigation_advances_without_reusing_completion_or_invalidating_on_error()
-> Result<(), Box<dyn Error>> {
    let mut gate = ReadinessGate::new(LaunchId::new(7)?, Generation::new(3)?);
    assert!(matches!(gate.accept(receipt(7, 3, false)?), ReadinessDecision::Accepted(_)));
    for generation in [2, 3] {
        assert_eq!(
            gate.navigate(Generation::new(generation)?),
            Err(NavigationError::NotIncreasing)
        );
    }
    assert_eq!(gate.navigation().get(), 3);
    assert_eq!(
        gate.accept(receipt(7, 3, false)?),
        ReadinessDecision::Rejected(ReceiptRejection::AlreadyCompleted)
    );
    gate.navigate(Generation::new(4)?)?;
    assert_eq!(
        gate.accept(receipt(7, 3, false)?),
        ReadinessDecision::Rejected(ReceiptRejection::WrongNavigation)
    );
    assert!(matches!(gate.accept(receipt(7, 4, false)?), ReadinessDecision::Accepted(_)));
    Ok(())
}

/// Cancellation closes all callbacks and reloads until a distinct launch is created.
#[test]
fn cancellation_is_permanent_and_retry_has_fresh_launch_identity() -> Result<(), Box<dyn Error>> {
    let mut gate = ReadinessGate::new(LaunchId::new(7)?, Generation::new(3)?);
    gate.cancel();
    gate.cancel();
    assert_eq!(
        gate.accept(receipt(7, 3, false)?),
        ReadinessDecision::Rejected(ReceiptRejection::Cancelled)
    );
    assert_eq!(gate.navigate(Generation::new(4)?), Err(NavigationError::Cancelled));
    let mut retry = ReadinessGate::new(LaunchId::new(8)?, Generation::new(1)?);
    assert_eq!(
        retry.accept(receipt(7, 3, false)?),
        ReadinessDecision::Rejected(ReceiptRejection::ForeignLaunch)
    );
    assert!(matches!(retry.accept(receipt(8, 1, false)?), ReadinessDecision::Accepted(_)));
    Ok(())
}
