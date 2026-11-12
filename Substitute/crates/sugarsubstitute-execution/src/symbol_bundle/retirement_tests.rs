// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve target custody after ACK and derive only actual original paired cleanup authority.
use super::test_support::*;
use crate::runtime_test_support::TestResult;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;
#[test]
fn acknowledged_main_keeps_custody_and_original_cleanup_cutoff_cannot_renew() -> TestResult {
    let mut pair = Pair::new()?;
    let origin = pair.rig.client.symbol_bundle_admission(&pair.context)?.into_origin();
    let completion = pair.finish_main()?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion);
    assert!(origin.check_effect().is_err());
    assert_eq!(pair.rig.scope.snapshot().reserved_bytes, 8159232);
    let (mut cleanup, context, deadline) = pair.activate()?;
    let ports = Arc::new(Ports(AtomicUsize::new(0)));
    let supplied = supplied(&context, context.memory().clone(), None, ports.clone());
    let first = pair.rig.client.symbol_bundle_retirement(&origin, &supplied)?;
    assert!(first.matches_origin(&origin));
    assert_eq!(first.cleanup_cutoff(), deadline.deadline());
    assert_eq!(first.original_context().now(), Moment::from_nanos(0));
    assert!(!first.original_context().cancellation_requested());
    pair.rig.clock.advance(10000000000);
    let second = pair.rig.client.symbol_bundle_retirement(&origin, &supplied)?;
    assert_eq!(first.cleanup_cutoff(), second.cleanup_cutoff());
    assert_eq!(ports.0.load(Ordering::Acquire), 0);
    pair.rig.clock.advance(5000000000);
    assert!(matches!(first.check(), Err(AdmissionError::DeadlineExpired { .. })));
    assert!(pair.rig.client.symbol_bundle_retirement(&origin, &supplied).is_err());
    assert_eq!(pair.rig.scope.snapshot().reserved_bytes, 8159232);
    assert!(matches!(pair.finish_cleanup(&mut cleanup)?.outcome, JobOutcome::TimedOut));
    Ok(())
}
#[test]
fn actual_reentrant_cleanup_scope_cancel_prevents_new_proof_and_preserves_target_claim()
-> TestResult {
    let mut pair = Pair::new()?;
    let origin = pair.rig.client.symbol_bundle_admission(&pair.context)?.into_origin();
    let (mut cleanup, context, _) = pair.activate()?;
    let core = pair.rig.client.core_for_test();
    let scope = pair.cleanup_scope.id();
    pair.rig.clock.on_next_current_thread_observation(Box::new(move || core.cancel_scope(scope)));
    assert!(matches!(
        pair.rig.client.symbol_bundle_retirement(&origin, &context),
        Err(AdmissionError::ScopeClosed { .. })
    ));
    let remainder = pair.context.memory().claim(8159232 - 2097152 - 4096)?;
    assert!(pair.context.memory().claim(1).is_err());
    assert!(origin.check_effect().is_ok());
    assert!(matches!(pair.finish_cleanup(&mut cleanup)?.outcome, JobOutcome::Cancelled));
    drop(remainder);
    Ok(())
}
