// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Competing campaigns use native exclusion without borrowing application or trust-writer locks.
use super::*;
use crate::update_repository::Failure as RepositoryFailure;
use std::io;
use sugarsubstitute_contracts::update::Channel;
fn worker(fixture: &Fixture) -> Result<Worker, Failure> {
    Ok(Worker::new(configuration(fixture, sugarsubstitute_update::http::Policy::loopback(1)?))?)
}
#[test]
fn live_preparation_excludes_both_channels_until_observed_retirement() -> TestResult {
    let mut first = Pipeline::new()?;
    let (fixture, _) = initialized(&mut first)?;
    let mut original = worker(&fixture)?;
    assert_eq!(step(&mut first, &mut original)?.progress, Progress::Checking);
    let root = fixture.installation.path().to_owned();
    // A preparation campaign does not reserve either installation activation or installer access.
    first.run(RESERVATION_BYTES, move |context| {
        let _run =
            InstallationRun::acquire(Arc::new(SelectionStore::open(&root, &context)?), &context)?;
        let _setup = InstallationSession::maintain(&root, &context)?;
        Ok(())
    })?;
    let mut second = Pipeline::new()?;
    for channel in [Channel::Release, Channel::Canary] {
        select_subscription(&fixture, channel)?;
        let mut competing = worker(&fixture)?;
        submit(&mut second, &mut competing)?;
        let observation = second.observe(|| competing.poll())?;
        assert_eq!(observation.progress, Progress::AlreadyPreparing);
        assert!(observation.failure.is_none());
        assert!(matches!(observation.outcome, JobOutcome::Success(())));
        assert_eq!(observation.progress, Progress::AlreadyPreparing);
        assert!(submit(&mut second, &mut competing).is_err());
        retire(&mut second, &mut competing)?;
    }
    retire(&mut first, &mut original)?;
    let mut resumed = worker(&fixture)?;
    assert_eq!(step(&mut second, &mut resumed)?.progress, Progress::Checking);
    retire(&mut second, &mut resumed)?;
    assert_unlocked(&mut second, &fixture)
}
#[test]
fn prepared_candidate_retains_exclusion_until_its_actual_disposal() -> TestResult {
    let mut first = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut first)?;
    let (mut original, server) = campaign(&mut first, &fixture, signed, true)?;
    for _ in 0..5 {
        step(&mut first, &mut original)?;
    }
    server.finish()?;
    let candidate = original.take_candidate()?;
    let mut second = Pipeline::new()?;
    let mut competing = worker(&fixture)?;
    submit(&mut second, &mut competing)?;
    let observation = second.observe(|| competing.poll())?;
    assert!(observation.failure.is_none());
    assert!(matches!(observation.outcome, JobOutcome::Success(())));
    assert_eq!(observation.progress, Progress::AlreadyPreparing);
    assert!(submit(&mut second, &mut competing).is_err());
    retire(&mut second, &mut competing)?;
    first.run(0, move |_| {
        drop(candidate);
        Ok(())
    })?;
    let mut resumed = worker(&fixture)?;
    assert_eq!(step(&mut second, &mut resumed)?.progress, Progress::Checking);
    retire(&mut second, &mut resumed)?;
    assert_unlocked(&mut first, &fixture)
}
#[test]
fn occupied_rendezvous_bytes_are_preserved_and_never_treated_as_a_stale_lock() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    let mut original = worker(&fixture)?;
    step(&mut pipeline, &mut original)?;
    retire(&mut pipeline, &mut original)?;
    let path = fixture.installation.path().join("state/update-trust").join("preparation-v1");
    let corruption = path.clone();
    pipeline.run(RESERVATION_BYTES, move |_| {
        std::fs::write(corruption, b"unknown owner bytes")?;
        Ok(())
    })?;
    let mut reopened = worker(&fixture)?;
    submit(&mut pipeline, &mut reopened)?;
    let observation = pipeline.observe(|| reopened.poll())?;
    assert_eq!(observation.progress, Progress::Failed);
    assert!(
        matches!(observation.failure.as_deref(), Some(RepositoryFailure::Trust(TrustFailure::Storage(cause))) if cause.kind() == io::ErrorKind::InvalidInput)
    );
    retire(&mut pipeline, &mut reopened)?;
    pipeline.run(RESERVATION_BYTES, move |_| {
        assert_eq!(std::fs::read(path)?, b"unknown owner bytes");
        Ok(())
    })?;
    fixture.preserved()
}
