// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject actual peer or wire budget mismatches before any authenticated launch observation.
use super::kernel::*;
use crate::{
    runtime_test_support::{Rig, TestResult},
    scope::lock,
};
use std::sync::atomic::Ordering;
use sugarsubstitute_contracts::execution::{CounterDeadline, JobOutcome};
fn rejected(configure: impl Fn(&Boundary) -> TestResult, writes: usize) -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    activate(&rig, &session)?;
    configure(&boundary)?;
    boundary.connect()?;
    let failures = rejected_connection(&rig, &session.control, None)?;
    assert!(!failures.is_empty());
    assert_eq!(boundary.writes.load(Ordering::Acquire), writes);
    assert!(lock(&boundary.replies).is_empty());
    session.control.close();
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Success(report) if report.authenticated_launches==0)
    );
    Ok(())
}
#[test]
fn actual_peer_session_mismatch_cannot_mint_authenticated_launch() -> TestResult {
    rejected(
        |boundary| {
            boundary.wrong_peer.store(true, Ordering::Release);
            Ok(())
        },
        0,
    )
}
#[test]
fn changed_fresh_connection_nonce_cannot_mint_authenticated_launch() -> TestResult {
    rejected(
        |boundary| {
            boundary.changed_nonce.store(true, Ordering::Release);
            Ok(())
        },
        1,
    )
}
#[test]
fn mismatched_source_qpc_frequency_rejects_before_challenge_or_cache() -> TestResult {
    rejected(
        |boundary| {
            *lock(&boundary.source) = CounterDeadline::new(30_000_001_000, 1_000_000_001)?;
            Ok(())
        },
        0,
    )
}
#[test]
fn far_future_source_cannot_poison_bounded_replay_floor() -> TestResult {
    rejected(
        |boundary| {
            *lock(&boundary.source) = CounterDeadline::new(3_600_000_001_001, 1_000_000_000)?;
            Ok(())
        },
        0,
    )
}
#[test]
fn configured_startup_horizon_survives_counter_transport_validation() -> TestResult {
    use sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget;
    for frequency in [10_000_000, 1_000_000_000] {
        let sample = 1000;
        let until = sample + frequency * EntrySetupBudget::DURATION.as_secs();
        crate::coordinator_entry::admission::validate_horizon(
            (sample, frequency),
            CounterDeadline::new(until as i64, frequency as i64)?,
        )?;
        assert!(
            crate::coordinator_entry::admission::validate_horizon(
                (sample, frequency),
                CounterDeadline::new(until as i64 + 1, frequency as i64)?
            )
            .is_err()
        );
    }
    Ok(())
}
