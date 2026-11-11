// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual shared module slot with genuine retained execution claims.
use super::*;
use crate::capture::tests::support::module_claim_context;
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A failed no-output load returns the identical original claim, and the slot is reusable.
#[test]
fn unused_load_returns_original_claim_without_another_owner() -> TestResult {
    let (fixture, context) = module_claim_context()?;
    let slot = Slot::new();
    let claim = context.memory().claim(65_536)?;
    let loan = LegacyLoan::reserve(&slot, claim).map_err(|_| "Unexpected occupied slot")?;
    assert_eq!(slot.legacy_bytes(), 0);
    let rejected = context.memory().claim(1)?;
    let rejected = match LegacyLoan::reserve(&slot, rejected) {
        Ok(_) => return Err("An active loan permitted a second module owner".into()),
        Err(original) => original,
    };
    assert_eq!(rejected.bytes(), 1);
    drop(rejected);
    let claim = loan.return_unused().ok_or("No-output load lost original claim")?;
    let (claim, bundle) = claim;
    assert!(bundle.is_none());
    assert_eq!(claim.bytes(), 65_536);
    let loan = LegacyLoan::reserve(&slot, claim).map_err(|_| "Returned slot stayed occupied")?;
    let (claim, bundle) = loan.return_unused().ok_or("Reusable slot lost original claim")?;
    assert!(bundle.is_none());
    drop(claim);
    drop(context);
    assert_eq!(fixture.finish(), 0);
    Ok(())
}

/// A callback panic retains exact module/PIN uncertainty and charge before returning outward.
#[test]
fn accepted_pin_panic_restores_original_owner_and_vetoes_second_module() -> TestResult {
    let (fixture, context) = module_claim_context()?;
    let slot = Slot::new();
    let original_reserved = context.memory().bytes();
    let claim = context.memory().claim(65_536)?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut loan = match LegacyLoan::reserve(&slot, claim) {
            Ok(value) => value,
            Err(_) => std::panic::resume_unwind(Box::new("Unexpected occupied fixture")),
        };
        // External fake loader output only: no Windows module was loaded or retired.
        loan.adopt_module(1);
        loan.pin_attempt();
        assert_eq!(slot.legacy_bytes(), 65_536);
        assert_eq!(slot.legacy_reservation(), original_reserved);
        std::panic::resume_unwind(Box::new("Original native callback panic"));
    }));
    let panic = result.err().ok_or("Original callback panic was swallowed")?;
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"Original native callback panic"));
    assert_eq!(slot.legacy_bytes(), 65_536);
    let rejected = context.memory().claim(1)?;
    let rejected = match LegacyLoan::reserve(&slot, rejected) {
        Ok(_) => return Err("Restored PIN owner allowed replacement".into()),
        Err(original) => original,
    };
    drop(rejected);
    assert!(Loan::take(&slot)?.is_none());
    drop(slot);
    drop(context);
    assert_eq!(fixture.finish(), 0);
    Ok(())
}

/// A zero PIN reply cannot manufacture no-effect after the actual attempted-call boundary.
#[test]
fn attempted_pin_vetoes_unused_return_even_without_an_output() -> TestResult {
    let (fixture, context) = module_claim_context()?;
    let slot = Slot::new();
    let claim = context.memory().claim(65_536)?;
    let mut loan = LegacyLoan::reserve(&slot, claim).map_err(|_| "Unexpected occupied slot")?;
    loan.pin_attempt();
    assert!(loan.return_unused().is_none());
    assert_eq!(slot.legacy_bytes(), 65_536);
    let rejected = context.memory().claim(1)?;
    let rejected = match LegacyLoan::reserve(&slot, rejected) {
        Ok(_) => return Err("Uncertain PIN owner was replaced".into()),
        Err(original) => original,
    };
    drop(rejected);
    drop(slot);
    drop(context);
    assert_eq!(fixture.finish(), 0);
    Ok(())
}
