// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual partner cancellation, worker confinement and physical dispatch limits.
use super::pair_test_support::{Release, Setup};
use crate::{
    runtime_test_support::{Gate, TestResult, spec},
    scope::lock,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::*;

#[test]
fn pair_keeps_one_ordinary_slot_and_allows_existing_lifecycle_slot() -> TestResult {
    let setup = Setup::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _guard = Release(release.clone());
    let (worker_entered, worker_release) = (entered.clone(), release.clone());
    let (mut source, mut receiving) = setup
        .submit(move |_, _| {
            worker_entered.open();
            worker_release.wait().map_err(|_| 1)?;
            Ok(true)
        })
        .map_err(|r| r.error)?;
    entered.wait()?;
    assert!(
        setup
            .rig
            .client
            .freeze_symbol_cleanup_deadline(
                &setup.source_cleanup,
                std::time::Duration::from_secs(15)
            )
            .is_err()
    );
    assert!(
        setup
            .rig
            .client
            .freeze_report_export_cleanup_deadline(
                &setup.receiving_cleanup,
                std::time::Duration::from_secs(15)
            )
            .is_err()
    );
    let called = Arc::new(AtomicBool::new(false));
    let worker_called = called.clone();
    let mut ordinary = setup.rig.client.submit_blocking(
        &setup.rig.scope,
        spec(5, JobClass::InfrastructureOrdinary, 100)?,
        move |_| {
            worker_called.store(true, Ordering::Release);
            Ok::<_, u8>(true)
        },
    )?;
    let mut lifecycle = setup.rig.client.submit_blocking(
        &setup.rig.scope,
        spec(6, JobClass::InfrastructureLifecycle, 100)?,
        move |_| Ok::<_, u8>(true),
    )?;
    assert!(matches!(setup.rig.completion(&mut lifecycle)?.outcome, JobOutcome::Success(true)));
    assert_eq!(ordinary.status().phase, JobPhase::Queued);
    assert!(!called.load(Ordering::Acquire));
    release.open();
    assert!(matches!(setup.rig.completion(&mut source)?.outcome, JobOutcome::Success(true)));
    assert!(matches!(setup.rig.completion(&mut receiving)?.outcome, JobOutcome::Success(true)));
    assert!(matches!(setup.rig.completion(&mut ordinary)?.outcome, JobOutcome::Success(true)));
    Ok(())
}

#[test]
fn both_effects_reject_foreign_thread_and_receiver_cancel_stops_both() -> TestResult {
    let setup = Setup::new()?;
    let stored = Arc::new(Mutex::new(None));
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _guard = Release(release.clone());
    let (worker_stored, worker_entered, worker_release) =
        (stored.clone(), entered.clone(), release.clone());
    let client = setup.rig.client.clone();
    let (mut source, mut receiving) = setup
        .submit(move |source, receiving| {
            let reader = client.committed_reader_admission(&source).map_err(|_| 1)?.into_origin();
            let export = client.report_export_admission(&receiving).map_err(|_| 2)?.into_origin();
            *lock(&worker_stored) = Some((reader.clone(), export.clone()));
            worker_entered.open();
            worker_release.wait().map_err(|_| 3)?;
            assert!(reader.check_effect().is_err());
            assert!(export.check_effect().is_err());
            Ok(true)
        })
        .map_err(|r| r.error)?;
    entered.wait()?;
    {
        let stored = lock(&stored);
        let (reader, export) = stored.as_ref().ok_or("Originals absent")?;
        assert!(reader.check_effect().is_err());
        assert!(export.check_effect().is_err());
    }
    receiving.cancel();
    release.open();
    assert!(matches!(setup.rig.completion(&mut source)?.outcome, JobOutcome::Cancelled));
    assert!(matches!(setup.rig.completion(&mut receiving)?.outcome, JobOutcome::Cancelled));
    drop(lock(&stored).take());
    Ok(())
}

#[test]
fn source_scope_cancellation_stops_receiving_effect_and_both_deliveries() -> TestResult {
    let setup = Setup::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _guard = Release(release.clone());
    let (worker_entered, worker_release) = (entered.clone(), release.clone());
    let client = setup.rig.client.clone();
    let (mut source, mut receiving) = setup
        .submit(move |_, receiving| {
            let origin = client.report_export_admission(&receiving).map_err(|_| 1)?.into_origin();
            worker_entered.open();
            worker_release.wait().map_err(|_| 2)?;
            assert!(origin.check_effect().is_err());
            Ok(true)
        })
        .map_err(|r| r.error)?;
    entered.wait()?;
    setup.rig.scope.cancel();
    release.open();
    assert!(matches!(setup.rig.completion(&mut source)?.outcome, JobOutcome::Cancelled));
    assert!(matches!(setup.rig.completion(&mut receiving)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn original_clock_expiry_caps_both_real_roles() -> TestResult {
    let setup = Setup::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _guard = Release(release.clone());
    let (worker_entered, worker_release) = (entered.clone(), release.clone());
    let client = setup.rig.client.clone();
    let (mut source, mut receiving) = setup
        .submit(move |source, receiving| {
            let reader = client.committed_reader_admission(&source).map_err(|_| 1)?.into_origin();
            let export = client.report_export_admission(&receiving).map_err(|_| 2)?.into_origin();
            worker_entered.open();
            worker_release.wait().map_err(|_| 3)?;
            assert!(matches!(reader.check_effect(), Err(AdmissionError::DeadlineExpired { .. })));
            assert!(matches!(export.check_effect(), Err(AdmissionError::DeadlineExpired { .. })));
            Ok(true)
        })
        .map_err(|r| r.error)?;
    entered.wait()?;
    setup.rig.clock.advance(10_000_000_000);
    release.open();
    assert!(matches!(setup.rig.completion(&mut source)?.outcome, JobOutcome::TimedOut));
    assert!(matches!(setup.rig.completion(&mut receiving)?.outcome, JobOutcome::TimedOut));
    Ok(())
}

#[test]
fn actual_owner_shutdown_waits_for_both_pair_roles() -> TestResult {
    let mut setup = Setup::new()?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _guard = Release(release.clone());
    let (worker_entered, worker_release) = (entered.clone(), release.clone());
    let (mut source, mut receiving) = setup
        .submit(move |_, _| {
            worker_entered.open();
            worker_release.wait().map_err(|_| 1)?;
            Ok(true)
        })
        .map_err(|r| r.error)?;
    entered.wait()?;
    setup.rig.owner.request_shutdown(Moment::from_nanos(15_000_000_000));
    assert_eq!(source.status().phase, JobPhase::Running);
    assert_eq!(receiving.status().phase, JobPhase::Running);
    release.open();
    assert!(matches!(setup.rig.completion(&mut source)?.outcome, JobOutcome::Cancelled));
    assert!(matches!(setup.rig.completion(&mut receiving)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
