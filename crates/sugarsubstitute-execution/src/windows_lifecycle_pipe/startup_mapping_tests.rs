// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test actual original borrow/count/claim ownership with an external immutable view fixture.
use super::*;
use crate::runtime_test_support::{Gate, Rig, TestResult, admitted_context, spec};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobOutcome};

/// Adopt only the owned external fixture's immutable bytes; no OS unmap is modeled.
fn mapped(rig: &Rig, claim: MemoryClaim, bytes: &[u8]) -> TestResult<StartupMapping> {
    let mut mapping = StartupMapping::new(claim, Signal::new(&rig.client.core_for_test()));
    // SAFETY: the test retains this immutable allocation through all real loans.
    // The actual borrow/count/claim owner never outlives the fixture bytes.
    unsafe { mapping.adopt(bytes.as_ptr() as usize, bytes.len()) }?;
    Ok(mapping)
}

/// Cache-only retention shares the original borrow; startup remains the only byte view.
#[test]
fn capture_retention_keeps_original_borrow_until_both_actual_loans_return() -> TestResult {
    let rig = Rig::new()?;
    let (scope, context) = admitted_context(&rig, 2, 1)?;
    let bytes = [1, 2, 3, 4];
    let mut mapping = mapped(&rig, context.memory().claim(4096)?, &bytes)?;
    let capture = mapping.loan(0..2)?.into_retention();
    let startup = mapping.loan(2..4)?;
    assert_eq!(startup.bytes()?, &[3, 4]);
    assert!(!mapping.can_unmap());
    assert!(context.memory().claim(1).is_err());
    drop(capture);
    assert!(!mapping.can_unmap());
    drop(startup);
    assert!(mapping.can_unmap());
    assert!(context.memory().claim(1).is_err());
    mapping.unmapped();
    drop(mapping);
    let reclaimed = context.memory().claim(4096)?;
    drop(reclaimed);
    drop(context);
    // Terminal/ACK does not establish disposal of the original blocking context.
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Physical owner loss does not manufacture metadata/charge release while a loan lives.
#[test]
fn last_retention_outlives_owner_and_returns_only_original_claim() -> TestResult {
    let rig = Rig::new()?;
    let (scope, context) = admitted_context(&rig, 2, 1)?;
    let bytes = [1, 2];
    let mut mapping = mapped(&rig, context.memory().claim(4096)?, &bytes)?;
    let retention = mapping.loan(0..2)?.into_retention();
    drop(mapping);
    assert!(context.memory().claim(1).is_err());
    drop(retention);
    let reclaimed = context.memory().claim(4096)?;
    drop(reclaimed);
    drop(context);
    // Terminal/ACK does not establish disposal of the original blocking context.
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Open the boundary before Rig joins on every assertion/error unwind.
struct Release(Arc<Gate>);
impl Drop for Release {
    /// No callback or native effect is needed to release the owned external barrier.
    fn drop(&mut self) {
        self.0.open();
    }
}

/// Actual existing workers drop both original aliases concurrently, without a new thread.
#[test]
fn concurrent_last_retention_and_owner_drop_keep_original_charge_until_controls_end() -> TestResult
{
    let rig = Rig::new()?;
    let release = Arc::new(Gate::default());
    let _release = Release(release.clone());
    let (scope, context) = admitted_context(&rig, 2, 1)?;
    let bytes = Arc::new([1, 2]);
    let mut mapping = mapped(&rig, context.memory().claim(4096)?, bytes.as_slice())?;
    let retention = mapping.loan(0..2)?.into_retention();
    let owner_entered = Arc::new(Gate::default());
    let loan_entered = Arc::new(Gate::default());
    let make_job = |id, class, entered: Arc<Gate>, action: Box<dyn FnOnce() + Send>| {
        let release = release.clone();
        let bytes = bytes.clone();
        rig.client
            .submit_blocking(&rig.scope, spec(id, class, 1)?, move |_| {
                entered.open();
                release.wait().map_err(|e| std::io::Error::other(e.to_string()))?;
                action();
                drop(bytes);
                Ok::<_, std::io::Error>(())
            })
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    };
    let mut owner_job = make_job(
        2,
        JobClass::InfrastructureOrdinary,
        owner_entered.clone(),
        Box::new(move || drop(mapping)),
    )?;
    let mut loan_job = make_job(
        3,
        JobClass::InfrastructureLifecycle,
        loan_entered.clone(),
        Box::new(move || drop(retention)),
    )?;
    owner_entered.wait()?;
    loan_entered.wait()?;
    assert!(context.memory().claim(1).is_err());
    release.open();
    assert!(matches!(rig.completion(&mut owner_job)?.outcome, JobOutcome::Success(())));
    assert!(matches!(rig.completion(&mut loan_job)?.outcome, JobOutcome::Success(())));
    let reclaimed = context.memory().claim(4096)?;
    drop(reclaimed);
    drop(context);
    // Terminal/ACK does not establish disposal of the original blocking context.
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
