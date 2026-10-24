// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the production preparation worker through actual executor admission and local HTTP.
mod cancellation;
mod capacity;
mod engine;
mod exclusion;
mod harness;
mod online_reopen;
mod reopen;
mod restart;
mod resumption;
mod startup;
mod subscription;
mod supersession;
use crate::setup_storage::{
    test_package::{maintenance::Fixture, repository::SignedPackage},
    test_worker::TestResult,
    *,
};
use std::sync::Arc;
use sugarsubstitute_update::fixture::NOW;
type Failure = Box<dyn std::error::Error + Send + Sync>;
use crate::{
    setup_storage::test_worker::pipeline::Pipeline,
    update_http::fixture::{Running, complete, exchange},
    update_preparation::{CachedConfiguration, Configuration, Observation, Progress, Worker},
};
use harness::*;
use sha2::{Digest, Sha256};
use std::time::Duration;
use sugarsubstitute_contracts::{
    execution::{JobOutcome, Moment},
    update::Admission,
};

#[test]
fn worker_stages_real_delivery_without_claiming_the_application_run_lock() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut pipeline)?;
    let (mut worker, server) = campaign(&mut pipeline, &fixture, signed, true)?;
    let root = fixture.installation.path().to_owned();
    let (store, run, before) = pipeline.run(RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(&root, &context)?);
        let run = InstallationRun::acquire(store.clone(), &context)?;
        let before = store.load(&context)?;
        Ok((store, run, before))
    })?;
    for expected in [
        Progress::Checking,
        Progress::Reopening,
        Progress::Downloading { received: 0, total: fixture.archive.len() as u64 },
        Progress::Preparing,
        Progress::Ready,
    ] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    server.finish()?;
    assert!(!worker.pending());
    let candidate = worker.take_candidate()?;
    assert_eq!(worker.progress(), Progress::Closed);
    pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        assert_eq!(
            store.load(&context)?.ok_or("No selection")?.accepted(),
            before.as_ref().and_then(|r| r.accepted())
        );
        assert!(candidate.transfer(|| NOW, &context, &mut None).is_err());
        assert!(InstallationRun::acquire(store.clone(), &context).is_err());
        drop(candidate);
        drop(run);
        assert!(InstallationRun::acquire(store, &context).is_ok());
        Ok(())
    })?;
    fixture.preserved()?;
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))?;
    Ok(())
}

#[test]
fn current_and_incompatible_offers_do_not_download_or_take_setup_exclusion() -> TestResult {
    for expected in [Admission::Current, Admission::Installer] {
        let mut pipeline = Pipeline::new()?;
        let (fixture, mut signed) = initialized(&mut pipeline)?;
        let fixture = match expected {
            Admission::Current => accepted(&mut pipeline, fixture)?,
            Admission::Installer => {
                requires(
                    &mut signed,
                    &fixture.archive,
                    "updater_protocol",
                    u64::from(sugarsubstitute_update::ENGINE_PROTOCOL) + 1,
                )?;
                fixture
            }
            Admission::Prepare | Admission::UpdateEngine => {
                return Err("Invalid decision fixture".into());
            }
        };
        let (mut worker, server) = campaign(&mut pipeline, &fixture, signed, false)?;
        step(&mut pipeline, &mut worker)?;
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::Decision(expected));
        server.finish()?;
        assert!(submit(&mut pipeline, &mut worker).is_err());
        assert!(worker.take_candidate().is_err());
        retire(&mut pipeline, &mut worker)?;
        assert_unlocked(&mut pipeline, &fixture)?;
    }
    Ok(())
}

#[test]
fn missing_trust_is_an_observed_failure_and_never_bootstraps_a_repository() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let fixture = pipeline.run(RESERVATION_BYTES, |context| {
        let fixture = Fixture::new(&context)?;
        SelectionStore::initialize(
            fixture.installation.path(),
            &sugarsubstitute_contracts::installation::InstallationRecord::empty(
                sugarsubstitute_contracts::installation::InstallationId::new("e".repeat(32))?,
            ),
            &context,
        )?;
        Ok(fixture)
    })?;
    let mut worker =
        Worker::new(configuration(&fixture, sugarsubstitute_update::http::Policy::loopback(1)?))?;
    submit(&mut pipeline, &mut worker)?;
    let observed = pipeline.observe(|| worker.poll())?;
    assert_eq!(observed.progress, Progress::Failed);
    let JobOutcome::Failure(cause) = &observed.outcome else {
        return Err("Failed preparation reported success to the execution owner".into());
    };
    assert!(Arc::ptr_eq(cause, observed.failure.as_ref().ok_or("Original cause absent")?));
    assert!(!fixture.installation.path().join(".update-trust").exists());
    assert!(submit(&mut pipeline, &mut worker).is_err());
    retire(&mut pipeline, &mut worker)?;
    assert_application_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}
