// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A repairing worker prepares the installation's own release again where an ordinary check
//! would stop, so a launcher that found nothing it could start has something to start.
use super::*;

/// Step `worker` until it has a candidate, failed, or decided not to prepare.
fn settle(pipeline: &mut Pipeline, worker: &mut Worker) -> Result<Progress, Failure> {
    for _ in 0..16 {
        let progress = step(pipeline, worker)?.progress;
        if matches!(progress, Progress::Ready | Progress::Failed | Progress::Decision(_)) {
            return Ok(progress);
        }
    }
    Err("The repair campaign did not settle".into())
}

#[test]
fn a_repair_prepares_the_release_the_installation_already_runs() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut pipeline)?;
    // An ordinary check of this installation answers "current" and downloads nothing.
    let fixture = accepted(&mut pipeline, fixture)?;
    // The package it was installed from is still in the installation's cache, so the repair
    // asks the feed only what the current release is and prepares from the cached bytes.
    let (mut worker, server) = repair_campaign(&mut pipeline, &fixture, signed, false)?;
    assert_eq!(settle(&mut pipeline, &mut worker)?, Progress::Ready);
    server.finish()?;
    drop(worker.take_candidate()?);
    Ok(())
}

#[test]
fn a_repair_prepares_a_release_that_bad_trials_withhold_from_ordinary_checks() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut pipeline)?;
    let root = fixture.installation.path().to_owned();
    pipeline.run(RESERVATION_BYTES, move |context| {
        crate::setup_storage::test_package::maintenance::strike_release(&root, 1, 2, &context)
    })?;
    let (mut worker, server) = repair_campaign(&mut pipeline, &fixture, signed, true)?;
    assert_eq!(settle(&mut pipeline, &mut worker)?, Progress::Ready);
    server.finish()?;
    drop(worker.take_candidate()?);
    Ok(())
}
