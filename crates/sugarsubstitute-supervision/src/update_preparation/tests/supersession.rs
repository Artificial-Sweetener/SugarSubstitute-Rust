// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! New HTTP publication supersedes pending selection and survives a fully offline restart.
use super::*;

#[test]
fn newer_remote_release_replaces_pending_publication_and_offline_restart_selects_the_replacement()
-> TestResult {
    let (fixture, previous) = restart::prepared()?;
    let receipt = std::fs::read(previous.join("application-update-v1.record"))?;
    let unknown = previous.join("user-note.txt");
    std::fs::write(&unknown, b"user file in previous generation")?;
    let mut pipeline = Pipeline::new()?;
    let signed = SignedPackage::new(&fixture.archive, 2)?;
    // The first campaign already persisted this archive. New metadata still requires discovery.
    let (mut worker, server) = campaign(&mut pipeline, &fixture, signed, false)?;
    for expected in [
        Progress::Checking,
        Progress::Downloading { received: 0, total: fixture.archive.len() as u64 },
        Progress::Preparing,
        Progress::Ready,
    ] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    server.finish()?;
    let candidate = worker.take_candidate()?;
    let replacement = candidate.directory().to_owned();
    assert_ne!(replacement, previous);
    pipeline.run(0, move |_| {
        drop(candidate);
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    drop((worker, pipeline));
    let mut pipeline = Pipeline::new()?;
    let mut resumed = Worker::resume_cached(configuration(
        &fixture,
        sugarsubstitute_update::http::Policy::loopback(1)?,
    ))?;
    for expected in [Progress::Restoring, Progress::Reopening, Progress::Ready] {
        assert_eq!(step(&mut pipeline, &mut resumed)?.progress, expected);
    }
    let candidate = resumed.take_candidate()?;
    assert_eq!(candidate.directory(), replacement);
    pipeline.run(0, move |_| {
        drop(candidate);
        Ok(())
    })?;
    assert_eq!(std::fs::read(previous.join("application-update-v1.record"))?, receipt);
    assert_eq!(std::fs::read(unknown)?, b"user file in previous generation");
    assert_unlocked(&mut pipeline, &fixture)
}
