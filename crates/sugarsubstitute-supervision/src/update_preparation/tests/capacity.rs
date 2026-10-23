// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify coordinator overlap against real admission ledgers without changing their limits.
use super::*;
use sugarsubstitute_contracts::execution::{AdmissionError, MemoryLease};

/// Model only the externally retained claims/registry footprint. Native run exclusion is real.
/// No process or claim table is fabricated by these accounting guards.
fn coordinator_storage(pipeline: &mut Pipeline) -> Result<MemoryLease, Failure> {
    pipeline.run(
        crate::CLAIM_POOL_BYTES + crate::WindowsProcessRegistry::required_reservation_bytes(),
        |context| Ok(context.memory().clone()),
    )
}

#[test]
fn same_ledger_cannot_admit_discovery_alongside_coordinator_minimum_custody() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    let coordinator = coordinator_storage(&mut pipeline)?;
    let root = fixture.installation.path().to_owned();
    let (store, run) = pipeline.run(LAUNCH_RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(&root, &context)?);
        let run = InstallationRun::acquire(store.clone(), &context)?;
        Ok((store, run))
    })?;
    let mut worker =
        Worker::new(configuration(&fixture, sugarsubstitute_update::http::Policy::loopback(1)?))?;
    assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::Checking);
    let Err(rejected) = submit_once(&mut pipeline, &mut worker) else {
        return Err("Discovery exceeded coordinator capacity".into());
    };
    let Some(cause) = rejected
        .downcast_ref::<std::io::Error>()
        .and_then(std::io::Error::get_ref)
        .and_then(|cause| cause.downcast_ref::<AdmissionError>())
    else {
        return Err(format!("Expected real admission saturation: {rejected:?}").into());
    };
    let requested = super::super::Progress::Checking.reservation();
    let retained = crate::CLAIM_POOL_BYTES
        + crate::WindowsProcessRegistry::required_reservation_bytes()
        + LAUNCH_RESERVATION_BYTES
        + TRUST_RESERVATION_BYTES;
    assert!(matches!(cause, AdmissionError::MemorySaturated { requested_bytes, available_bytes }
        if *requested_bytes == requested && *available_bytes == 256 * 1024 * 1024 - retained));
    assert!(!worker.pending());
    assert_eq!(worker.progress(), Progress::Checking);
    retire(&mut pipeline, &mut worker)?;
    pipeline.run(RESERVATION_BYTES, move |context| {
        assert!(InstallationRun::acquire(store, &context).is_err());
        drop((run, coordinator));
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)
}

#[test]
fn isolated_ledger_prepares_while_coordinator_capacity_and_native_run_remain_owned() -> TestResult {
    let mut parent = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut parent)?;
    let coordinator = coordinator_storage(&mut parent)?;
    let root = fixture.installation.path().to_owned();
    let (store, run) = parent.run(LAUNCH_RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(&root, &context)?);
        let run = InstallationRun::acquire(store.clone(), &context)?;
        Ok((store, run))
    })?;
    // A second test ledger represents the proposed helper's independent process budget.
    // This qualifies accounting and native exclusion, not process creation or teardown.
    let mut helper = Pipeline::new()?;
    let (mut worker, server) = campaign(&mut helper, &fixture, signed, true)?;
    for expected in [
        Progress::Checking,
        Progress::Downloading { received: 0, total: fixture.archive.len() as u64 },
        Progress::Preparing,
        Progress::Ready,
    ] {
        assert_eq!(step(&mut helper, &mut worker)?.progress, expected);
    }
    server.finish()?;
    let candidate = worker.take_candidate()?;
    helper.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        assert!(candidate.transfer(|| NOW, &context, &mut None).is_err());
        drop(candidate);
        Ok(())
    })?;
    parent.run(RESERVATION_BYTES, move |context| {
        assert!(InstallationRun::acquire(store.clone(), &context).is_err());
        drop(run);
        assert!(InstallationRun::acquire(store, &context).is_ok());
        drop(coordinator);
        Ok(())
    })?;
    helper.run(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))?;
    assert_unlocked(&mut parent, &fixture)
}
