// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify lifecycle identity shape, clock bounds and redacted original causes.
use std::error::Error;
use sugarsubstitute_contracts::{execution::Moment, lifecycle::*};

#[test]
fn durable_namespace_and_epoch_shapes_and_registry_generations_are_distinct()
-> Result<(), IdentityError> {
    assert_eq!(NamespaceId::new([42; 32]).bytes(), &[42; 32]);
    assert_eq!(CoordinatorEpoch::new([0; 16]), Err(IdentityError::InvalidEpoch));
    assert_eq!(CoordinatorEpoch::new([3; 16])?.bytes(), &[3; 16]);
    assert_eq!(RunGeneration::new(0), Err(IdentityError::Zero));
    assert_eq!(RunGeneration::new(1)?.next()?.get(), 2);
    assert_eq!(ResourceGeneration::new(u64::MAX)?.next(), Err(IdentityError::Exhausted));
    assert_eq!(RunGeneration::new(u64::MAX)?.next(), Err(IdentityError::Exhausted));
    Ok(())
}

#[test]
fn one_shutdown_budget_uses_exact_grace_and_total_intervals_and_rejects_overflow()
-> Result<(), LifecycleRejection> {
    let budget = ShutdownBudget::new(Moment::from_nanos(7))?;
    assert_eq!(budget.graceful_until.as_nanos(), 10_000_000_007);
    assert_eq!(budget.settle_until.as_nanos(), 15_000_000_007);
    assert_eq!(
        ShutdownBudget::new(Moment::from_nanos(u64::MAX)),
        Err(LifecycleRejection::InvalidClock)
    );
    Ok(())
}

#[test]
fn lifecycle_failures_preserve_concrete_causes_without_plaintext_debug_or_display()
-> Result<(), Box<dyn Error>> {
    let failure = LifecycleFailure::new(
        LifecycleFailureCode::Cleanup,
        Box::new(std::io::Error::other("credential-bearing original")),
    );
    assert!(!format!("{failure:?} {failure}").contains("credential"));
    let source = failure.source().ok_or("Missing original cause")?;
    assert!(source.downcast_ref::<std::io::Error>().is_some());
    assert_eq!(source.to_string(), "credential-bearing original");
    Ok(())
}
