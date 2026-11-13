// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove the selection reader-writer lock against a real second handle on manual time.
//!
//! The test thread holds the lock file through its own handle. The storage job's wait is a
//! kernel lock request, so it wakes on release with the clock never advanced, and it times out
//! only when the driver moves time to the wait's bound.
use super::*;
use crate::setup_storage::{
    short_lock::WAIT,
    test_worker::driven::{Driver, Handoff, run_driven},
};
use std::{
    fs::OpenOptions,
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::{Clock, Moment};

const RESERVATION: u64 = 16 * 1024 * 1024;
/// The job's store is initialized and the lock file exists.
const READY: u32 = 1;
/// The driver holds its handle on the lock file.
const HELD: u32 = 2;
/// The call under test returned.
const DECIDED: u32 = 3;
/// The driver released its handle after observing the outcome.
const RELEASED: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Held {
    Shared,
    Exclusive,
}
/// What one store call under test returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    Done,
    Timeout,
    Cancelled,
    Other,
}
impl<T> From<&Result<T, SelectionFailure>> for Outcome {
    fn from(result: &Result<T, SelectionFailure>) -> Self {
        match result {
            Ok(_) => Self::Done,
            Err(SelectionFailure::LockTimeout) => Self::Timeout,
            Err(SelectionFailure::LockCancelled) => Self::Cancelled,
            Err(_) => Self::Other,
        }
    }
}
/// The store call that waits on the driver's handle.
#[derive(Clone, Copy, Debug)]
enum Operation {
    /// A pure read: shared lock.
    Load,
    /// A compare-and-publish: exclusive lock.
    Publish,
}
/// How the driver ends the wait.
#[derive(Clone, Copy, Debug)]
enum Ending {
    Release,
    Deadline,
    Cancel,
}

fn lock_path(root: &Path) -> PathBuf {
    root.join("state").join(DIRECTORY).join(LOCK)
}

fn hold(path: &Path, kind: Held) -> Result<std::fs::File, Box<dyn Error + Send + Sync>> {
    let file = OpenOptions::new().read(true).write(true).share_mode(0x1 | 0x2).open(path)?;
    match kind {
        Held::Shared => file.try_lock_shared()?,
        Held::Exclusive => file.try_lock()?,
    }
    Ok(file)
}

/// Run `operation` against a lock the driver holds as `held`, ended by `ending`.
/// Returns the call's outcome and whether the record then holds the published successor.
fn exercise(
    held: Held,
    operation: Operation,
    ending: Ending,
) -> Result<(Outcome, bool), Box<dyn Error + Send + Sync>> {
    let root = root()?;
    let path = root.path().to_path_buf();
    let worker_path = path.clone();
    let result = Arc::new(Mutex::new(None));
    let worker_result = result.clone();
    run_driven(
        RESERVATION,
        move |context, handoff: &Handoff| {
            let initial = initial()?;
            let store = SelectionStore::initialize(&worker_path, &initial, context)?;
            let successor = initial.prepare(
                generation(1)?,
                PreparationIntent::Automatic,
                crate::setup_storage::selection::unix_now()?,
            )?;
            handoff.reach(READY);
            handoff.wait(HELD);
            let outcome = match operation {
                Operation::Load => Outcome::from(&store.load(context)),
                Operation::Publish => {
                    Outcome::from(&store.compare_exchange(&initial, &successor, context))
                }
            };
            handoff.reach(DECIDED);
            handoff.wait(RELEASED);
            let published =
                outcome != Outcome::Cancelled && store.load(context)? == Some(successor);
            *crate::registry::lock(&worker_result) = Some((outcome, published));
            Ok(())
        },
        |driver: &Driver<'_>| {
            driver.handoff.wait(READY);
            let mut holder = Some(hold(&lock_path(&path), held)?);
            driver.handoff.reach(HELD);
            let bound = Moment::from_nanos(0).checked_add(WAIT)?;
            let conflicts = held == Held::Exclusive || matches!(operation, Operation::Publish);
            if conflicts {
                driver.clock.wait_for_sleeper(bound);
                match ending {
                    Ending::Release => drop(holder.take()),
                    Ending::Deadline => driver.clock.advance_to(bound)?,
                    Ending::Cancel => driver.cancel(),
                }
            }
            driver.handoff.wait(DECIDED);
            drop(holder);
            driver.handoff.reach(RELEASED);
            if !matches!(ending, Ending::Deadline) || !conflicts {
                assert_eq!(driver.clock.now(), Moment::from_nanos(0));
            }
            Ok(())
        },
    )?;
    crate::registry::lock(&result).take().ok_or_else(|| "Lock outcome absent".into())
}

#[test]
fn a_reader_waits_out_an_in_flight_publication_and_then_reads() -> TestResult {
    assert_eq!(
        exercise(Held::Exclusive, Operation::Load, Ending::Release)?,
        (Outcome::Done, false)
    );
    Ok(())
}

#[test]
fn a_writer_waits_for_an_in_flight_reader_and_then_publishes() -> TestResult {
    assert_eq!(exercise(Held::Shared, Operation::Publish, Ending::Release)?, (Outcome::Done, true));
    assert_eq!(
        exercise(Held::Exclusive, Operation::Publish, Ending::Release)?,
        (Outcome::Done, true)
    );
    Ok(())
}

#[test]
fn concurrent_readers_proceed_together_without_waiting() -> TestResult {
    assert_eq!(exercise(Held::Shared, Operation::Load, Ending::Release)?, (Outcome::Done, false));
    Ok(())
}

#[test]
fn a_lock_held_past_the_bound_is_a_typed_timeout_for_readers_and_writers() -> TestResult {
    assert_eq!(
        exercise(Held::Exclusive, Operation::Load, Ending::Deadline)?,
        (Outcome::Timeout, false)
    );
    assert_eq!(
        exercise(Held::Shared, Operation::Publish, Ending::Deadline)?,
        (Outcome::Timeout, false)
    );
    Ok(())
}

#[test]
fn cancellation_ends_reader_and_writer_waits_with_a_typed_cancel() -> TestResult {
    let (reader, _) = exercise(Held::Exclusive, Operation::Load, Ending::Cancel)?;
    let (writer, published) = exercise(Held::Shared, Operation::Publish, Ending::Cancel)?;
    assert_eq!((reader, writer, published), (Outcome::Cancelled, Outcome::Cancelled, false));
    Ok(())
}
