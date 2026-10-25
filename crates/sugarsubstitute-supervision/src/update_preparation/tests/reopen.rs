// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Completed preparation starts from protected native files independently of provisioning caches.
use super::*;
use std::os::windows::fs::OpenOptionsExt;
use sugarsubstitute_update::fixture::{EXPIRES, key, root};

/// Construct the real cache-only worker without a network configuration.
fn cached(fixture: &Fixture) -> Result<Worker, Failure> {
    Ok(Worker::resume_cached(cached_configuration(fixture))?)
}

#[test]
fn completed_candidate_reopens_with_inaccessible_archive_and_unusable_extraction_workspace()
-> TestResult {
    let (fixture, generation) = restart::prepared()?;
    let archive = crate::setup_storage::test_worker::cache_file(
        fixture.installation.path(),
        crate::setup_storage::cache::Namespace::UpdateDownloads,
        "payload",
    )?;
    let _exclusive =
        std::fs::OpenOptions::new().read(true).write(true).share_mode(0).open(&archive)?;
    assert!(std::fs::read(&archive).is_err());
    let workspace = fixture.workspace.path().join("reserved-user-file");
    std::fs::write(&workspace, b"preserve this file")?;
    let mut config = cached_configuration(&fixture);
    config.workspace = workspace.clone();
    let mut worker = Worker::resume_cached(config)?;
    let mut pipeline = Pipeline::new()?;
    for expected in [Progress::Restoring, Progress::Reopening, Progress::Ready] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    let candidate = worker.take_candidate()?;
    assert_eq!(candidate.directory(), generation);
    for leaf in [
        "sugarsubstitute.exe",
        "application-package.json",
        "application-update-v1.record",
        "startup.local.toml",
        "setup-generation.json",
    ] {
        assert!(std::fs::write(generation.join(leaf), b"must stay pinned").is_err(), "{leaf}");
    }
    assert_eq!(std::fs::read(&workspace)?, b"preserve this file");
    pipeline.run(0, move |_| {
        drop(candidate);
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    fixture.preserved()
}

#[test]
fn corrupt_completed_records_are_preserved_without_reextracting_or_publishing() -> TestResult {
    for leaf in [
        "application-package.json",
        "application-update-v1.record",
        "setup-generation.json",
        "installation-ready.json",
        "sugarsubstitute-launch.local.json",
    ] {
        let (fixture, generation) = restart::prepared()?;
        let selection =
            fixture.installation.path().join(".selection-state/.substitute-selection.json");
        let original = std::fs::read(&selection)?;
        let damaged = generation.join(leaf);
        std::fs::write(&damaged, b"preserve damaged record")?;
        let mut pipeline = Pipeline::new()?;
        let mut worker = cached(&fixture)?;
        for expected in [Progress::Restoring, Progress::Reopening] {
            assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
        }
        submit(&mut pipeline, &mut worker)?;
        let observed = pipeline.observe(|| worker.poll())?;
        assert_eq!(observed.progress, Progress::Failed, "{leaf}");
        assert!(matches!(observed.outcome, JobOutcome::Failure(_)), "{leaf}");
        assert!(worker.take_candidate().is_err());
        assert_eq!(std::fs::read(&damaged)?, b"preserve damaged record");
        assert_eq!(std::fs::read(&selection)?, original);
        retire(&mut pipeline, &mut worker)?;
        assert_unlocked(&mut pipeline, &fixture)?;
        fixture.preserved()?;
    }
    Ok(())
}

#[test]
fn publication_from_another_generation_cannot_authorize_a_completed_candidate() -> TestResult {
    let (fixture, generation) = restart::prepared()?;
    let (_foreign, foreign_generation) = restart::prepared()?;
    let record = generation.join("application-update-v1.record");
    let foreign = std::fs::read(foreign_generation.join("application-update-v1.record"))?;
    std::fs::write(&record, &foreign)?;
    let mut pipeline = Pipeline::new()?;
    let mut worker = cached(&fixture)?;
    step(&mut pipeline, &mut worker)?;
    step(&mut pipeline, &mut worker)?;
    submit(&mut pipeline, &mut worker)?;
    let observed = pipeline.observe(|| worker.poll())?;
    assert_eq!(observed.progress, Progress::Failed);
    assert!(worker.take_candidate().is_err());
    assert_eq!(std::fs::read(&record)?, foreign);
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)
}

#[test]
fn authority_expiring_after_cached_discovery_cannot_return_a_reopened_candidate() -> TestResult {
    let (fixture, _) = restart::prepared()?;
    let mut pipeline = Pipeline::new()?;
    let mut worker = cached(&fixture)?;
    for expected in [Progress::Restoring, Progress::Reopening] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    pipeline.admit(|client, scope, id| {
        Ok(worker.submit(client, scope, id, Moment::from_nanos(2_000_000_000), || EXPIRES)?)
    })?;
    let observed = pipeline.observe(|| worker.poll())?;
    assert_eq!(observed.progress, Progress::Failed);
    assert!(worker.take_candidate().is_err());
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)
}

#[test]
fn reopened_candidate_rechecks_expiry_and_rotated_authority_before_trial() -> TestResult {
    for rotate in [false, true] {
        let (fixture, _) = restart::prepared()?;
        let mut pipeline = Pipeline::new()?;
        let mut worker = cached(&fixture)?;
        for expected in [Progress::Restoring, Progress::Reopening, Progress::Ready] {
            assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
        }
        let candidate = worker.take_candidate()?;
        let installation = fixture.installation.path().to_owned();
        if rotate {
            let mut authority = Pipeline::new()?;
            let installation = installation.clone();
            authority.run(TRUST_RESERVATION_BYTES, move |context| {
                let trust = TrustStore::open(&installation, [92; 32], &context)?;
                trust.rotate(&root(&key(93)?, 2, Some(&key(92)?))?, &context)?;
                Ok(())
            })?;
        }
        pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
            let store = Arc::new(SelectionStore::open(&installation, &context)?);
            let before = store.load(&context)?;
            assert!(
                candidate
                    .transfer(|| if rotate { NOW } else { EXPIRES }, &context, &mut None)
                    .is_err()
            );
            assert_eq!(store.load(&context)?, before);
            drop(candidate);
            assert!(InstallationRun::acquire(store, &context).is_ok());
            Ok(())
        })?;
        assert_unlocked(&mut pipeline, &fixture)?;
        fixture.preserved()?;
    }
    Ok(())
}

#[test]
fn cancelling_each_cached_startup_boundary_releases_custody_without_changing_selection()
-> TestResult {
    for repair in [false, true] {
        for completed in 0..=if repair { 5 } else { 3 } {
            let (fixture, generation) = restart::prepared()?;
            if repair {
                std::fs::remove_file(generation.join("sugarsubstitute.exe"))?;
            }
            let selection =
                fixture.installation.path().join(".selection-state/.substitute-selection.json");
            let original = std::fs::read(&selection)?;
            let mut pipeline = Pipeline::new()?;
            let mut worker = cached(&fixture)?;
            for _ in 0..completed {
                step(&mut pipeline, &mut worker)?;
            }
            retire(&mut pipeline, &mut worker)?;
            assert_eq!(std::fs::read(&selection)?, original);
            assert_unlocked(&mut pipeline, &fixture)?;
            fixture.preserved()?;
        }
    }
    Ok(())
}
