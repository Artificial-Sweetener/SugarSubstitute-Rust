// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A second launch holding the selection lock past its wait bound never costs a trial its
//! acceptance or verdict: the publication waits again until the holder lets go or it is cancelled.
use super::*;
use crate::setup_storage::{
    selection::{DIRECTORY, LOCK, RECORD},
    short_lock::WAIT,
    test_worker::driven::{Driver, Handoff, run_driven},
};
use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt, path::Path, sync::Mutex};
use sugarsubstitute_contracts::execution::Moment;

const RESERVATION: u64 = 16 * 1024 * 1024;
const READY: u32 = 1;
const HELD: u32 = 2;
const DECIDED: u32 = 3;

/// The trial publication under contention.
#[derive(Clone, Copy)]
enum Publication {
    Accept,
    Settle,
}
/// How the driver ends its hold after the publication has waited out several bounds.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Ending {
    Release,
    Cancel,
}

/// Hold the lock shared, as a second launch reading the record does.
fn hold(installation: &Path) -> Result<std::fs::File, Box<dyn std::error::Error + Send + Sync>> {
    let path = installation.join("state").join(DIRECTORY).join(LOCK);
    let file = OpenOptions::new().read(true).write(true).share_mode(0x1 | 0x2).open(path)?;
    file.try_lock_shared()?;
    Ok(file)
}

/// Run `publication` while the driver holds the lock through `bounds` lock-wait bounds, then
/// ends the hold. Returns the publication's outcome and the record it leaves.
fn contended(
    publication: Publication,
    bounds: u32,
    ending: Ending,
) -> Result<
    (Result<InstallationRecord, String>, InstallationRecord),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let root = root()?;
    let path = root.path().to_path_buf();
    let worker_path = path.clone();
    let observed = Arc::new(Mutex::new(None));
    let worker_observed = observed.clone();
    run_driven(
        RESERVATION,
        move |context, handoff: &Handoff| {
            let store = Arc::new(SelectionStore::initialize(&worker_path, &initial()?, context)?);
            let prepared = prepare(&store, &initial()?, 1, context)?;
            let mut run = InstallationRun::acquire(store.clone(), context)?;
            let trial = run.begin_trial(&prepared, context)?;
            handoff.reach(READY);
            handoff.wait(HELD);
            let outcome = match publication {
                Publication::Accept => run.accept(&trial, context),
                Publication::Settle => run.finish_trial(&trial, NO_VERDICT, context),
            };
            handoff.reach(DECIDED);
            // Read the bytes directly: a cancelled job can no longer take the record lock.
            let selection = worker_path.join("state").join(DIRECTORY);
            let record = InstallationRecord::decode(&std::fs::read(selection.join(RECORD))?)?;
            *crate::registry::lock(&worker_observed) =
                Some((outcome.map_err(|cause| format!("{cause:?}")), record));
            Ok(())
        },
        |driver: &Driver<'_>| {
            driver.handoff.wait(READY);
            let holder = hold(&path)?;
            driver.handoff.reach(HELD);
            let mut bound = Moment::from_nanos(0);
            for _ in 0..bounds {
                bound = bound.checked_add(WAIT)?;
                driver.clock.wait_for_sleeper(bound);
                driver.clock.advance_to(bound)?;
            }
            driver.clock.wait_for_sleeper(bound.checked_add(WAIT)?);
            match ending {
                Ending::Release => drop(holder),
                Ending::Cancel => {
                    driver.cancel();
                    driver.handoff.wait(DECIDED);
                    drop(holder);
                }
            }
            driver.handoff.wait(DECIDED);
            Ok(())
        },
    )?;
    crate::registry::lock(&observed).take().ok_or_else(|| "Publication outcome absent".into())
}

#[test]
fn acceptance_waits_out_a_second_launch_holding_the_record_past_several_bounds() -> TestResult {
    let (outcome, record) = contended(Publication::Accept, 3, Ending::Release)?;
    let accepted = outcome?;
    assert_eq!(accepted.accepted(), Some(&generation(1)?));
    assert_eq!(record, accepted);
    Ok(())
}

#[test]
fn settlement_waits_out_contention_and_publishes_no_verdict() -> TestResult {
    let (outcome, record) = contended(Publication::Settle, 2, Ending::Release)?;
    let released = outcome?;
    assert_eq!(released.candidate(), Some(&Candidate::Prepared { generation: generation(1)? }));
    assert_eq!(record, released);
    Ok(())
}

#[test]
fn cancelled_acceptance_leaves_the_trial_unpublished_for_a_no_verdict_settlement() -> TestResult {
    let (outcome, record) = contended(Publication::Accept, 1, Ending::Cancel)?;
    assert_eq!(outcome, Err(format!("{:?}", SelectionFailure::LockCancelled)));
    assert!(matches!(record.candidate(), Some(Candidate::Trial { .. })));
    Ok(())
}
