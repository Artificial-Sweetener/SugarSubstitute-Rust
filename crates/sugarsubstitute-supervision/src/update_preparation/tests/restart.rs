// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recreate the campaign and execution owner after all earlier preparation custody has retired.
use super::*;

/// Prepare through real HTTP, then retire every worker before returning durable fixture state.
pub(super) fn prepared() -> Result<(Fixture, std::path::PathBuf), Failure> {
    let mut pipeline = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut pipeline)?;
    let (mut worker, server) = campaign(&mut pipeline, &fixture, signed, true)?;
    for _ in 0..4 {
        step(&mut pipeline, &mut worker)?;
    }
    server.finish()?;
    let candidate = worker.take_candidate()?;
    let path = candidate.directory().to_owned();
    pipeline.run(0, move |_| {
        drop(candidate);
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    drop((worker, pipeline));
    Ok((fixture, path))
}

#[test]
fn offline_restart_reauthenticates_publication_and_resumes_the_same_prepared_generation()
-> TestResult {
    let (fixture, original) = prepared()?;
    let mut pipeline = Pipeline::new()?;
    let mut worker = Worker::resume_cached(configuration(
        &fixture,
        sugarsubstitute_update::http::Policy::loopback(1)?,
    ))?;
    for expected in [Progress::Restoring, Progress::Reopening, Progress::Ready] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    let candidate = worker.take_candidate()?;
    assert_eq!(candidate.directory(), original);
    pipeline.run(0, move |_| {
        drop(candidate);
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}

#[test]
fn cached_restart_preserves_modified_payload_and_configuration_without_publishing_a_candidate()
-> TestResult {
    for leaf in ["sugarsubstitute.exe", "startup.local.toml"] {
        let (fixture, generation) = prepared()?;
        let selection =
            fixture.installation.path().join(".selection-state/.substitute-selection.json");
        let before = std::fs::read(&selection)?;
        let changed = generation.join(leaf);
        let bytes = b"modified staged data must remain intact";
        std::fs::write(&changed, bytes)?;
        let mut pipeline = Pipeline::new()?;
        let mut worker = Worker::resume_cached(configuration(
            &fixture,
            sugarsubstitute_update::http::Policy::loopback(1)?,
        ))?;
        for expected in [Progress::Restoring, Progress::Reopening] {
            assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
        }
        submit(&mut pipeline, &mut worker)?;
        let observed = pipeline.observe(|| worker.poll())?;
        assert_eq!(observed.progress, Progress::Failed);
        assert!(matches!(observed.outcome, JobOutcome::Failure(_)));
        assert!(observed.failure.is_some());
        assert!(worker.take_candidate().is_err());
        assert_eq!(std::fs::read(&changed)?, bytes);
        assert_eq!(std::fs::read(&selection)?, before);
        retire(&mut pipeline, &mut worker)?;
        assert_unlocked(&mut pipeline, &fixture)?;
        fixture.preserved()?;
    }
    Ok(())
}

#[test]
fn cached_restart_repairs_missing_payload_in_place_and_preserves_unknown_files() -> TestResult {
    let (fixture, generation) = prepared()?;
    let selection = fixture.installation.path().join(".selection-state/.substitute-selection.json");
    let before = std::fs::read(&selection)?;
    let payload = generation.join("sugarsubstitute.exe");
    let original = std::fs::read(&payload)?;
    std::fs::remove_file(&payload)?;
    let unknown = generation.join(".payload-stage-interrupted");
    std::fs::write(&unknown, b"unrecognized partial output")?;
    let mut pipeline = Pipeline::new()?;
    let mut worker = Worker::resume_cached(configuration(
        &fixture,
        sugarsubstitute_update::http::Policy::loopback(1)?,
    ))?;
    for expected in [
        Progress::Restoring,
        Progress::Reopening,
        Progress::RestoringArchive,
        Progress::Preparing,
        Progress::Ready,
    ] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    let candidate = worker.take_candidate()?;
    assert_eq!(candidate.directory(), generation);
    assert_eq!(std::fs::read(&payload)?, original);
    assert_eq!(std::fs::read(&selection)?, before);
    assert_eq!(std::fs::read(&unknown)?, b"unrecognized partial output");
    pipeline.run(0, move |_| {
        drop(candidate);
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    fixture.preserved()
}

#[test]
fn offline_check_reads_acceptance_after_the_worker_has_opened_trust() -> TestResult {
    let (fixture, _) = prepared()?;
    let mut pipeline = Pipeline::new()?;
    let mut worker = Worker::resume_cached(configuration(
        &fixture,
        sugarsubstitute_update::http::Policy::loopback(1)?,
    ))?;
    assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::Restoring);
    accept_pending(&mut pipeline, &fixture)?;
    assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::Decision(Admission::Current));
    assert!(worker.take_candidate().is_err());
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)
}

#[test]
fn malformed_accepted_identity_stops_online_and_offline_checks_without_claiming_current()
-> TestResult {
    let (fixture, generation) = prepared()?;
    let mut pipeline = Pipeline::new()?;
    accept_pending(&mut pipeline, &fixture)?;
    let receipt = generation.join("application-update-v1.record");
    std::fs::write(&receipt, b"damaged receipt")?;
    for cached in [false, true] {
        let config = configuration(&fixture, sugarsubstitute_update::http::Policy::loopback(1)?);
        let mut worker = if cached { Worker::resume_cached(config)? } else { Worker::new(config)? };
        step(&mut pipeline, &mut worker)?;
        submit(&mut pipeline, &mut worker)?;
        let observed = pipeline.observe(|| worker.poll())?;
        assert_eq!(observed.progress, Progress::Failed);
        let cause = observed.failure.ok_or("Receipt error missing")?;
        let crate::update_repository::Failure::Trust(TrustFailure::Storage(storage)) =
            cause.as_ref()
        else {
            return Err(format!("Expected receipt storage failure: {cause:?}").into());
        };
        assert_eq!(storage.kind(), std::io::ErrorKind::InvalidData);
        assert!(storage.to_string().contains("Update trust record is damaged"), "{storage}");
        assert!(matches!(observed.outcome, JobOutcome::Failure(_)));
        assert_eq!(std::fs::read(&receipt)?, b"damaged receipt");
        retire(&mut pipeline, &mut worker)?;
        assert_unlocked(&mut pipeline, &fixture)?;
    }
    Ok(())
}

#[test]
fn changing_subscription_does_not_reuse_other_channels_cached_release_or_current_decision()
-> TestResult {
    let (fixture, _) = prepared()?;
    let mut pipeline = Pipeline::new()?;
    accept_pending(&mut pipeline, &fixture)?;
    select_subscription(&fixture, sugarsubstitute_contracts::update::Channel::Canary)?;
    let mut worker = Worker::resume_cached(configuration(
        &fixture,
        sugarsubstitute_update::http::Policy::loopback(1)?,
    ))?;
    step(&mut pipeline, &mut worker)?;
    assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::Unavailable);
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)
}

#[test]
fn missing_and_partial_offline_evidence_never_asserts_current_or_downloads() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    let mut missing = Worker::resume_cached(configuration(
        &fixture,
        sugarsubstitute_update::http::Policy::loopback(1)?,
    ))?;
    step(&mut pipeline, &mut missing)?;
    assert_eq!(step(&mut pipeline, &mut missing)?.progress, Progress::Unavailable);
    assert!(submit(&mut pipeline, &mut missing).is_err());
    retire(&mut pipeline, &mut missing)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    drop((pipeline, fixture));
    let (fixture, generation) = prepared()?;
    let payload = crate::setup_storage::test_worker::cache_file(
        fixture.installation.path(),
        crate::setup_storage::cache::Namespace::UpdateDownloads,
        "payload",
    )?;
    std::fs::write(&payload, &fixture.archive[..7])?;
    std::fs::remove_file(generation.join("sugarsubstitute.exe"))?;
    let mut pipeline = Pipeline::new()?;
    let mut partial = Worker::resume_cached(configuration(
        &fixture,
        sugarsubstitute_update::http::Policy::loopback(1)?,
    ))?;
    step(&mut pipeline, &mut partial)?;
    assert_eq!(step(&mut pipeline, &mut partial)?.progress, Progress::Reopening);
    assert_eq!(step(&mut pipeline, &mut partial)?.progress, Progress::RestoringArchive);
    assert_eq!(step(&mut pipeline, &mut partial)?.progress, Progress::Unavailable);
    assert_eq!(std::fs::read(payload)?, &fixture.archive[..7]);
    retire(&mut pipeline, &mut partial)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}

#[test]
fn offline_restart_rejects_expired_authority_and_corrupt_repair_archive() -> TestResult {
    let (fixture, _) = prepared()?;
    let mut pipeline = Pipeline::new()?;
    let mut expired = Worker::resume_cached(configuration(
        &fixture,
        sugarsubstitute_update::http::Policy::loopback(1)?,
    ))?;
    step(&mut pipeline, &mut expired)?;
    pipeline.admit(|client, scope, id| {
        Ok(expired.submit(client, scope, id, Moment::from_nanos(2_000_000_000), || {
            sugarsubstitute_update::fixture::EXPIRES + 1
        })?)
    })?;
    let observed = pipeline.observe(|| expired.poll())?;
    assert_eq!(observed.progress, Progress::Failed);
    assert!(observed.failure.is_some());
    retire(&mut pipeline, &mut expired)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    // Expiry advances the clock floor; a separate installation isolates byte tampering from it.
    drop((pipeline, fixture));
    let (fixture, generation) = prepared()?;
    let payload = crate::setup_storage::test_worker::cache_file(
        fixture.installation.path(),
        crate::setup_storage::cache::Namespace::UpdateDownloads,
        "payload",
    )?;
    let mut damaged = fixture.archive.clone();
    damaged[0] ^= 1;
    std::fs::write(&payload, damaged)?;
    std::fs::remove_file(generation.join("sugarsubstitute.exe"))?;
    let mut pipeline = Pipeline::new()?;
    let mut corrupt = Worker::resume_cached(configuration(
        &fixture,
        sugarsubstitute_update::http::Policy::loopback(1)?,
    ))?;
    step(&mut pipeline, &mut corrupt)?;
    assert_eq!(step(&mut pipeline, &mut corrupt)?.progress, Progress::Reopening);
    assert_eq!(step(&mut pipeline, &mut corrupt)?.progress, Progress::RestoringArchive);
    submit(&mut pipeline, &mut corrupt)?;
    let observed = pipeline.observe(|| corrupt.poll())?;
    assert_eq!(observed.progress, Progress::Failed);
    assert!(observed.failure.is_some());
    assert!(corrupt.take_candidate().is_err());
    retire(&mut pipeline, &mut corrupt)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}
