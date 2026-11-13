// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Online reauthorization reuses completed native files and refreshes offline evidence.
use super::*;
use std::os::windows::fs::OpenOptionsExt;
use sugarsubstitute_update::fixture::{EXPIRES, Set, key};

#[test]
fn online_recheck_reopens_without_archive_or_workspace_and_refreshes_offline_evidence() -> TestResult
{
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
    let mut signed = SignedPackage::new(&fixture.archive, 1)?;
    signed.metadata = Set::targets(
        &key(92)?,
        [2; 3],
        &[
            (&signed.request.target_name(), &signed.descriptor),
            (&format!("{:x}.ssupdate", Sha256::digest(&fixture.archive)), &fixture.archive),
        ],
        EXPIRES,
    )?;
    let mut pipeline = Pipeline::new()?;
    let exchanges = discovery(&signed)?;
    let (server, policy) =
        pipeline.run(8 * 1024 * 1024, move |context| Running::start(exchanges, &context))?;
    let mut config = configuration(&fixture, policy);
    config.workspace = workspace.clone();
    let mut worker = Worker::new(config)?;
    for expected in
        [Progress::Checking, Progress::Reopening, Progress::Remembering, Progress::Ready]
    {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    server.finish()?;
    let candidate = worker.take_candidate()?;
    assert_eq!(candidate.directory(), generation);
    pipeline.run(0, move |_| {
        drop(candidate);
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    let mut config = cached_configuration(&fixture);
    config.workspace = workspace.clone();
    let mut offline = Worker::resume_cached(config)?;
    for expected in [Progress::Restoring, Progress::Reopening, Progress::Ready] {
        assert_eq!(step(&mut pipeline, &mut offline)?.progress, expected);
    }
    let candidate = offline.take_candidate()?;
    assert_eq!(candidate.directory(), generation);
    pipeline.run(0, move |_| {
        drop(candidate);
        Ok(())
    })?;
    assert_eq!(std::fs::read(workspace)?, b"preserve this file");
    assert_unlocked(&mut pipeline, &fixture)?;
    fixture.preserved()
}

#[test]
fn online_recheck_preserves_damaged_completed_payload_without_reextracting() -> TestResult {
    let (fixture, generation) = restart::prepared()?;
    let damaged = generation.join("sugarsubstitute.exe");
    std::fs::write(&damaged, b"preserve damaged executable")?;
    let mut pipeline = Pipeline::new()?;
    let signed = SignedPackage::new(&fixture.archive, 1)?;
    let (mut worker, server) = campaign(&mut pipeline, &fixture, signed, false)?;
    for expected in [Progress::Checking, Progress::Reopening] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    submit(&mut pipeline, &mut worker)?;
    let observed = pipeline.observe(|| worker.poll())?;
    assert_eq!(observed.progress, Progress::Failed);
    assert!(matches!(observed.outcome, JobOutcome::Failure(_)));
    server.finish()?;
    retire(&mut pipeline, &mut worker)?;
    assert_eq!(std::fs::read(damaged)?, b"preserve damaged executable");
    assert_unlocked(&mut pipeline, &fixture)
}
