// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove rejected pairs retain their input and never partially charge or pair either role.
use super::pair_test_support::{Release, Setup};
use crate::{
    runtime_test_support::{Gate, Rig, TestResult, spec},
    scope::lock,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;

#[test]
fn mismatch_returns_identical_factory_without_partial_pairing() -> TestResult {
    let setup = Setup::new()?;
    let calls = Arc::new(AtomicUsize::new(0));
    let worker_calls = calls.clone();
    let factory = move |_: JobContext, _: JobContext| {
        worker_calls.fetch_add(1, Ordering::AcqRel);
        Ok::<_, u8>(true)
    };
    let mut wrong = setup.receiving_spec;
    wrong.deadline = Some(Moment::from_nanos(9_000_000_000));
    let rejected = setup
        .rig
        .client
        .submit_report_export_pair(
            crate::ReportExportPairRequest::new(
                &setup.rig.scope,
                setup.source_spec,
                &setup.source_cleanup,
                &setup.receiving_scope,
                wrong,
                &setup.receiving_cleanup,
            ),
            factory,
        )
        .err()
        .ok_or("Mismatched cutoff admitted")?;
    assert!(matches!(rejected.error, AdmissionError::InvalidReportExportAdmission));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert_eq!(setup.rig.scope.snapshot().reserved_bytes, 0);
    assert_eq!(setup.receiving_scope.snapshot().reserved_bytes, 0);
    let (mut source, mut receiving) = setup.submit(rejected.factory).map_err(|r| r.error)?;
    assert!(matches!(setup.rig.completion(&mut source)?.outcome, JobOutcome::Success(true)));
    assert!(matches!(setup.rig.completion(&mut receiving)?.outcome, JobOutcome::Success(true)));
    assert_eq!(calls.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn foreign_cleanup_scope_and_prior_pairing_reject() -> TestResult {
    let setup = Setup::new()?;
    let foreign = Setup::new()?;
    assert!(
        setup
            .rig
            .client
            .submit_report_export_pair(
                crate::ReportExportPairRequest::new(
                    &setup.rig.scope,
                    setup.source_spec,
                    &foreign.source_cleanup,
                    &setup.receiving_scope,
                    setup.receiving_spec,
                    &setup.receiving_cleanup
                ),
                |_, _| Ok::<_, u8>(true)
            )
            .is_err()
    );
    assert!(
        setup
            .rig
            .client
            .submit_report_export_pair(
                crate::ReportExportPairRequest::new(
                    &setup.rig.scope,
                    setup.source_spec,
                    &setup.source_cleanup,
                    &foreign.receiving_scope,
                    setup.receiving_spec,
                    &setup.receiving_cleanup
                ),
                |_, _| Ok::<_, u8>(true)
            )
            .is_err()
    );
    let (mut source, mut receiving) = setup.submit(|_, _| Ok(true)).map_err(|r| r.error)?;
    let source_done = setup.rig.completion(&mut source)?;
    let receiving_done = setup.rig.completion(&mut receiving)?;
    assert!(matches!(source_done.outcome, JobOutcome::Success(true)));
    assert!(matches!(receiving_done.outcome, JobOutcome::Success(true)));
    assert!(setup.submit(|_, _| Ok(true)).is_err());
    Ok(())
}

#[test]
fn pair_control_storage_is_bounded_inside_existing_source_metadata() -> TestResult {
    let setup = Setup::new()?;
    let huge = [7_u8; 4096];
    let factory = move |_: JobContext, _: JobContext| Ok::<_, [u8; 4096]>(huge[0] == 7);
    let rejected = setup
        .rig
        .client
        .submit_report_export_pair(
            crate::ReportExportPairRequest::new(
                &setup.rig.scope,
                setup.source_spec,
                &setup.source_cleanup,
                &setup.receiving_scope,
                setup.receiving_spec,
                &setup.receiving_cleanup,
            ),
            factory,
        )
        .err()
        .ok_or("Oversized pair control admitted")?;
    assert!(matches!(rejected.error, AdmissionError::InvalidReportExportAdmission));
    assert_eq!(setup.rig.scope.snapshot().reserved_bytes, 0);
    assert_eq!(setup.receiving_scope.snapshot().reserved_bytes, 0);
    assert_eq!(setup.source_cleanup.status().phase, JobPhase::Queued);
    assert_eq!(setup.receiving_cleanup.status().phase, JobPhase::Queued);
    Ok(())
}

#[test]
fn queued_receiver_cancel_disposes_both_without_calling_factory() -> TestResult {
    let rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _guard = Release(release.clone());
    let (worker_entered, worker_release) = (entered.clone(), release.clone());
    let mut blocker = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 100)?,
        move |_| {
            worker_entered.open();
            worker_release.wait().map_err(|_| 1)?;
            Ok::<_, u8>(true)
        },
    )?;
    entered.wait()?;
    let mut setup = Setup::after(rig, 2)?;
    setup.release = Some(release.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    let worker_calls = calls.clone();
    let (mut source, mut receiving) = setup
        .submit(move |_, _| {
            worker_calls.fetch_add(1, Ordering::AcqRel);
            Ok(true)
        })
        .map_err(|r| r.error)?;
    assert_eq!(source.status().phase, JobPhase::Queued);
    assert_eq!(receiving.status().phase, JobPhase::Queued);
    let state = lock(&setup.rig.client.core.state);
    assert_eq!(state.queue.iter().map(crate::task::QueuedJob::queue_slots).sum::<usize>(), 4);
    drop(state);
    receiving.cancel();
    assert!(matches!(setup.rig.completion(&mut source)?.outcome, JobOutcome::Cancelled));
    assert!(matches!(setup.rig.completion(&mut receiving)?.outcome, JobOutcome::Cancelled));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    release.open();
    assert!(matches!(setup.rig.completion(&mut blocker)?.outcome, JobOutcome::Success(true)));
    Ok(())
}

#[test]
fn combined_original_bytes_reject_without_spending_either_cleanup() -> TestResult {
    let rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _guard = Release(release.clone());
    let (worker_entered, worker_release) = (entered.clone(), release.clone());
    let bytes = 256 * 1024 * 1024 - 65_536 - 8_290_304 + 1;
    let mut blocker = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureLifecycle, bytes)?,
        move |_| {
            worker_entered.open();
            worker_release.wait().map_err(|_| 1)?;
            Ok::<_, u8>(true)
        },
    )?;
    entered.wait()?;
    let mut setup = Setup::after(rig, 2)?;
    setup.release = Some(release.clone());
    let rejected = setup.submit(|_, _| Ok(true)).err().ok_or("Combined limit exceeded")?;
    assert!(matches!(
        rejected.error,
        AdmissionError::MemorySaturated { requested_bytes: 8_290_304, .. }
    ));
    assert_eq!(setup.rig.scope.snapshot().reserved_bytes, bytes);
    assert_eq!(setup.receiving_scope.snapshot().reserved_bytes, 0);
    release.open();
    drop(setup.rig.completion(&mut blocker)?);
    let core = setup.rig.client.core_for_test();
    drop(
        core.cpu_changed
            .wait_while(lock(&core.state), |state| state.bytes != 65_536)
            .map_err(|error| std::io::Error::other(error.to_string()))?,
    );
    let (mut source, mut receiving) = setup.submit(rejected.factory).map_err(|r| r.error)?;
    assert!(matches!(setup.rig.completion(&mut source)?.outcome, JobOutcome::Success(true)));
    assert!(matches!(setup.rig.completion(&mut receiving)?.outcome, JobOutcome::Success(true)));
    Ok(())
}

#[test]
fn two_queue_credits_are_enforced_at_existing_ordinary_bound() -> TestResult {
    let rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _guard = Release(release.clone());
    let (worker_entered, worker_release) = (entered.clone(), release.clone());
    let mut blocker = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 100)?,
        move |_| {
            worker_entered.open();
            worker_release.wait().map_err(|_| 1)?;
            Ok::<_, u8>(true)
        },
    )?;
    entered.wait()?;
    let mut setup = Setup::after_main(rig, 2, 100)?;
    setup.release = Some(release.clone());
    let mut jobs = Vec::new();
    for id in 4..31 {
        jobs.push(setup.rig.client.submit_blocking(
            &setup.rig.scope,
            spec(id, JobClass::InfrastructureOrdinary, 100)?,
            |_| Ok::<_, u8>(true),
        )?);
    }
    let rejected = setup.submit(|_, _| Ok(true)).err().ok_or("Two queue credits not enforced")?;
    assert!(matches!(rejected.error, AdmissionError::QueueSaturated { .. }));
    assert_eq!(setup.receiving_scope.snapshot().reserved_bytes, 0);
    release.open();
    drop(setup.rig.completion(&mut blocker)?);
    for job in &mut jobs {
        assert!(matches!(setup.rig.completion(job)?.outcome, JobOutcome::Success(true)));
    }
    Ok(())
}
