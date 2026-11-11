// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise actual admission provenance, original ports and retained close custody.

use super::{origin::RESERVATION_BYTES, test_support::*};
use crate::{
    runtime_test_support::{Rig, TestResult},
    scope::lock,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;

#[test]
fn same_numeric_foreign_memory_is_rejected_before_clock_and_claim() -> TestResult {
    let rig = Rig::new()?;
    let held = HeldCapture::new(&rig, 1, RESERVATION_BYTES, 10_000_000_000)?;
    let ports = Arc::new(SubstitutedPorts { reads: AtomicUsize::new(0) });
    let fake = substituted(
        &held.context,
        MemoryLease::new(RESERVATION_BYTES, Box::new(ForeignRelease)),
        ports.clone(),
    );
    let reads = Arc::new(AtomicUsize::new(0));
    let callback = reads.clone();
    rig.clock.on_next_current_thread_observation(Box::new(move || {
        callback.fetch_add(1, Ordering::AcqRel);
    }));
    assert!(matches!(
        rig.client.capture_bundle_admission(&fake),
        Err(AdmissionError::InvalidCaptureAdmission)
    ));
    assert_eq!(reads.load(Ordering::Acquire), 0);
    assert_eq!(ports.reads.load(Ordering::Acquire), 0);
    let actual = rig.client.capture_bundle_admission(&held.context)?;
    assert_eq!(reads.load(Ordering::Acquire), 1);
    assert!(actual.check().is_ok());
    Ok(())
}

#[test]
fn same_actual_memory_reconstructs_core_ports_and_issues_claim_once() -> TestResult {
    let rig = Rig::new()?;
    let held = HeldCapture::new(&rig, 1, RESERVATION_BYTES, 10_000_000_000)?;
    let ports = Arc::new(SubstitutedPorts { reads: AtomicUsize::new(0) });
    let supplied = substituted(&held.context, held.context.memory().clone(), ports.clone());
    let admission = rig.client.capture_bundle_admission(&supplied)?;
    assert_eq!(admission.original_context()?.now(), Moment::from_nanos(0));
    assert!(!admission.original_context()?.cancellation_requested());
    assert_eq!(ports.reads.load(Ordering::Acquire), 0);
    assert!(matches!(
        rig.client.capture_bundle_admission(&held.context),
        Err(AdmissionError::InvalidCaptureAdmission)
    ));
    let remainder = held.context.memory().claim(2_097_152)?;
    assert!(held.context.memory().claim(1).is_err());
    assert_eq!(remainder.reserved_bytes(), RESERVATION_BYTES);
    let origin = admission.into_origin();
    assert!(origin.control_storage_bytes() <= 65_536);
    assert_eq!(origin.retained_bytes(), 2_097_152);
    assert_eq!(origin.reservation_bytes(), RESERVATION_BYTES);
    Ok(())
}

#[test]
fn actual_reentrant_scope_cancel_vetoes_issuance_after_clock_callback() -> TestResult {
    let rig = Rig::new()?;
    let held = HeldCapture::new(&rig, 1, RESERVATION_BYTES, 10_000_000_000)?;
    let core = rig.client.core_for_test();
    let scope_id = rig.scope.id();
    rig.clock.on_next_current_thread_observation(Box::new(move || core.cancel_scope(scope_id)));
    assert!(matches!(
        rig.client.capture_bundle_admission(&held.context),
        Err(AdmissionError::ScopeClosed { .. })
    ));
    assert!(held.context.memory().claim(RESERVATION_BYTES).is_ok());
    assert!(
        !lock(&rig.client.core_for_test().state)
            .jobs
            .get(&held.context.job_id())
            .ok_or("Actual record absent")?
            .capture_bundle_issued
    );
    Ok(())
}

#[test]
fn over_reserved_actual_job_cannot_issue_bundle_or_consume_capacity() -> TestResult {
    let rig = Rig::new()?;
    let held = HeldCapture::new(&rig, 1, RESERVATION_BYTES + 1, 10_000_000_000)?;
    assert!(matches!(
        rig.client.capture_bundle_admission(&held.context),
        Err(AdmissionError::InvalidCaptureAdmission)
    ));
    assert!(held.context.memory().claim(RESERVATION_BYTES + 1).is_ok());
    Ok(())
}

#[test]
fn terminal_ack_keeps_original_claim_but_separate_actual_close_remains_valid() -> TestResult {
    let rig = Rig::new()?;
    let mut held = HeldCapture::new(&rig, 1, RESERVATION_BYTES, 10_000_000_000)?;
    let origin = rig.client.capture_bundle_admission(&held.context)?.into_origin();
    let completion = held.finish(&rig)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion);
    assert!(matches!(origin.check_effect(), Err(AdmissionError::InvalidCaptureAdmission)));
    assert_eq!(rig.scope.snapshot().reserved_bytes, RESERVATION_BYTES);
    let close = HeldCapture::new(&rig, 2, 4096, 30_000_000_000)?;
    let proof = rig.client.capture_bundle_retirement(&origin, &close.context)?;
    assert!(proof.matches_origin(&origin));
    assert_eq!(proof.cleanup_cutoff(), Moment::from_nanos(15_000_000_000));
    assert!(proof.check().is_ok());
    Ok(())
}

#[test]
fn original_cleanup_cutoff_never_renews_and_fake_ports_cannot_extend_it() -> TestResult {
    let rig = Rig::new()?;
    let held = HeldCapture::new(&rig, 1, RESERVATION_BYTES, 10_000_000_000)?;
    let origin = rig.client.capture_bundle_admission(&held.context)?.into_origin();
    let close = HeldCapture::new(&rig, 2, 4096, 30_000_000_000)?;
    let ports = Arc::new(SubstitutedPorts { reads: AtomicUsize::new(0) });
    let supplied = substituted(&close.context, close.context.memory().clone(), ports.clone());
    let first = rig.client.capture_bundle_retirement(&origin, &supplied)?;
    rig.clock.advance(10_000_000_000);
    assert!(matches!(origin.check_effect(), Err(AdmissionError::DeadlineExpired { .. })));
    let second = rig.client.capture_bundle_retirement(&origin, &supplied)?;
    assert_eq!(first.cleanup_cutoff(), second.cleanup_cutoff());
    assert_eq!(ports.reads.load(Ordering::Acquire), 0);
    rig.clock.advance(5_000_000_000);
    assert!(
        matches!(first.check(), Err(AdmissionError::DeadlineExpired { deadline, .. }) if deadline == first.cleanup_cutoff())
    );
    assert!(matches!(
        rig.client.capture_bundle_retirement(&origin, &supplied),
        Err(AdmissionError::DeadlineExpired { .. })
    ));
    assert_eq!(rig.scope.snapshot().reserved_bytes, RESERVATION_BYTES + 4096);
    Ok(())
}

#[test]
fn origin_claim_drops_once_and_final_ledger_release_follows_actual_worker_join() -> TestResult {
    let rig = Rig::new()?;
    let mut held = HeldCapture::new(&rig, 1, RESERVATION_BYTES, 10_000_000_000)?;
    let origin = rig.client.capture_bundle_admission(&held.context)?.into_origin();
    let final_alias = origin.clone();
    let remainder = held.context.memory().claim(2_097_152)?;
    drop(origin);
    assert!(held.context.memory().claim(1).is_err());
    drop(final_alias);
    let reusable = held.context.memory().claim(2_097_152)?;
    drop(reusable);
    drop(remainder);
    let origin = {
        // Issuance remains sticky even after claim capacity becomes reusable.
        assert!(matches!(
            rig.client.capture_bundle_admission(&held.context),
            Err(AdmissionError::InvalidCaptureAdmission)
        ));
        held.context.memory().clone()
    };
    drop(held.finish(&rig)?);
    drop(held);
    let Rig { owner, client, scope, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, RESERVATION_BYTES);
    assert!(lock(&client.core_for_test().state).jobs.is_empty());
    drop(origin);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn actual_clock_failure_preserves_original_typed_cause_and_never_claims_bundle() -> TestResult {
    let rig = Rig::new()?;
    let held = HeldCapture::new(&rig, 1, RESERVATION_BYTES, 10_000_000_000)?;
    rig.clock.on_next_current_thread_observation(Box::new(|| {
        std::panic::resume_unwind(Box::new(TimeError::Regression));
    }));
    let error = match rig.client.capture_bundle_admission(&held.context) {
        Err(error) => error,
        Ok(_) => return Err("Fatal original clock issued capture authority".into()),
    };
    assert!(matches!(error, AdmissionError::Unexpected(_)));
    let mut cause: Option<&dyn std::error::Error> = Some(&error);
    let mut original = false;
    while let Some(current) = cause {
        original |= current.downcast_ref::<TimeError>() == Some(&TimeError::Regression);
        cause = current.source();
    }
    assert!(original);
    assert!(held.context.memory().claim(RESERVATION_BYTES).is_ok());
    assert!(lock(&rig.client.core_for_test().state).stopping);
    Ok(())
}

#[test]
fn concurrent_actual_origin_alias_drops_return_only_the_original_claim() -> TestResult {
    use crate::runtime_test_support::{Gate, spec};
    let rig = Rig::new()?;
    let held = HeldCapture::new(&rig, 1, RESERVATION_BYTES, 10_000_000_000)?;
    let first = rig.client.capture_bundle_admission(&held.context)?.into_origin();
    let second = first.clone();
    let remainder = held.context.memory().claim(2_097_152)?;
    let arrivals = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _release = DropRelease(release.clone());
    let first_task = OriginDropTask {
        origin: Some(first),
        arrivals: arrivals.clone(),
        entered: entered.clone(),
        release: release.clone(),
    };
    let second_task = OriginDropTask {
        origin: Some(second),
        arrivals: arrivals.clone(),
        entered: entered.clone(),
        release: release.clone(),
    };
    let mut first_job =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuInteractive, 4096)?, move |_| {
            first_task
        })?;
    let mut second_job =
        rig.client
            .submit_cpu(&rig.scope, spec(3, JobClass::CpuDecode, 4096)?, move |_| second_task)?;
    entered.wait()?;
    assert!(held.context.memory().claim(1).is_err());
    release.open();
    assert!(matches!(rig.completion(&mut first_job)?.outcome, JobOutcome::Success(())));
    assert!(matches!(rig.completion(&mut second_job)?.outcome, JobOutcome::Success(())));
    assert_eq!(arrivals.load(Ordering::Acquire), 2);
    let reused = held.context.memory().claim(2_097_152)?;
    assert!(held.context.memory().claim(1).is_err());
    drop(reused);
    drop(remainder);
    Ok(())
}

/// Repeated cleanup cannot mask the first genuine clock failure or invoke a new provider.
#[test]
fn cleanup_clock_failure_remains_original_after_actual_owner_shutdown() -> TestResult {
    let rig = Rig::new()?;
    let held = HeldCapture::new(&rig, 1, RESERVATION_BYTES, 10_000_000_000)?;
    let origin = rig.client.capture_bundle_admission(&held.context)?.into_origin();
    let close = HeldCapture::new(&rig, 2, 4096, 30_000_000_000)?;
    let observations = Arc::new(AtomicUsize::new(0));
    let first = observations.clone();
    rig.clock.on_next_current_thread_observation(Box::new(move || {
        first.fetch_add(1, Ordering::AcqRel);
        std::panic::resume_unwind(Box::new(TimeError::Regression));
    }));
    let initial = rig
        .client
        .capture_bundle_retirement(&origin, &close.context)
        .err()
        .ok_or("Failed original cleanup clock created proof")?;
    let repeated = observations.clone();
    rig.clock.on_next_current_thread_observation(Box::new(move || {
        repeated.fetch_add(1, Ordering::AcqRel);
    }));
    let later = rig
        .client
        .capture_bundle_retirement(&origin, &close.context)
        .err()
        .ok_or("Repeated failed cleanup clock created proof")?;
    for error in [&initial, &later] {
        assert!(matches!(error, AdmissionError::Unexpected(_)));
        let mut cause: Option<&dyn std::error::Error> = Some(error);
        let mut original = false;
        while let Some(current) = cause {
            original |= current.downcast_ref::<TimeError>() == Some(&TimeError::Regression);
            cause = current.source();
        }
        assert!(original);
    }
    assert_eq!(observations.load(Ordering::Acquire), 1);
    assert!(lock(&rig.client.core_for_test().state).stopping);
    assert_eq!(rig.scope.snapshot().reserved_bytes, RESERVATION_BYTES + 4096);
    Ok(())
}

/// Original cutoff failure stays primary after actual scope stop and a later fatal Core clock.
#[test]
fn cleanup_arithmetic_failure_is_sticky_without_new_cutoff() -> TestResult {
    let rig = Rig::new()?;
    let held = HeldCapture::new(&rig, 1, RESERVATION_BYTES, u64::MAX)?;
    let origin = rig.client.capture_bundle_admission(&held.context)?.into_origin();
    let close = HeldCapture::new(&rig, 2, 4096, u64::MAX)?;
    rig.clock.advance(u64::MAX - 1);
    let initial = rig
        .client
        .capture_bundle_retirement(&origin, &close.context)
        .err()
        .ok_or("Overflowed original cutoff created proof")?;
    rig.scope.cancel();
    rig.clock.on_next_current_thread_observation(Box::new(|| {
        std::panic::resume_unwind(Box::new(TimeError::Regression));
    }));
    let later_clock = origin.check_effect().err().ok_or("Later fatal clock allowed effects")?;
    assert!(matches!(later_clock, AdmissionError::Unexpected(_)));
    assert!(lock(&rig.client.core_for_test().state).stopping);
    let later = rig
        .client
        .capture_bundle_retirement(&origin, &close.context)
        .err()
        .ok_or("Repeated overflowed cleanup created proof")?;
    for error in [&initial, &later] {
        assert!(matches!(error, AdmissionError::Unexpected(_)));
        let mut cause: Option<&dyn std::error::Error> = Some(error);
        let mut original = false;
        while let Some(current) = cause {
            original |= current.downcast_ref::<TimeError>() == Some(&TimeError::Overflow);
            cause = current.source();
        }
        assert!(original);
    }
    assert_eq!(rig.scope.snapshot().reserved_bytes, RESERVATION_BYTES + 4096);
    Ok(())
}
