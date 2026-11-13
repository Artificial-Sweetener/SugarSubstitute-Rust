// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove real kernel lock waits wake on release, time out and cancel on the job's own clock.
use super::*;
use crate::{
    ExecutionClient, ExecutionOwner,
    clock_fixture::ManualClock,
    runtime_test_support::{TestResult, Wake, spec},
};
use std::{fs::File, sync::Arc};
use std::{
    fs::OpenOptions,
    num::NonZeroUsize,
    os::windows::{fs::OpenOptionsExt, io::AsHandle},
    path::{Path, PathBuf},
    sync::Mutex,
};
use sugarsubstitute_contracts::execution::{
    Clock, ExecutionPort, JobClass, JobCompletion, OwnedJob, Scope, ScopeId, ScopeSpec,
    TerminalKind,
};

/// One owned temporary directory per test, removed with its rendezvous file.
struct Directory(PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "sugarsubstitute-file-lock-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(cause) = std::fs::remove_dir_all(&self.0) {
            tracing::warn!(operation = "file_lock_test_cleanup", %cause, "Test directory kept");
        }
    }
}

const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;
const FILE_SHARE_READ_WRITE: u32 = 0x1 | 0x2;

/// Observable wait result; the typed error itself is not `Clone`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Observed {
    Granted,
    Timeout,
    Cancelled,
    Unavailable,
    InvalidInput,
    OtherIo,
}
impl From<Result<(), FileLockError>> for Observed {
    fn from(result: Result<(), FileLockError>) -> Self {
        match result {
            Ok(()) => Self::Granted,
            Err(FileLockError::Timeout) => Self::Timeout,
            Err(FileLockError::Cancelled) => Self::Cancelled,
            Err(FileLockError::Unavailable) => Self::Unavailable,
            Err(FileLockError::Io(cause)) if cause.kind() == std::io::ErrorKind::InvalidInput => {
                Self::InvalidInput
            }
            Err(FileLockError::Io(_)) => Self::OtherIo,
        }
    }
}

/// The waiter's stored result, written before its job reports terminal.
type Cell = Arc<Mutex<Option<Observed>>>;

struct Harness {
    clock: Arc<ManualClock>,
    wake: Arc<Wake>,
    client: ExecutionClient,
    scope: Scope,
    _owner: ExecutionOwner,
    directory: Directory,
    next: u64,
}
impl Harness {
    fn new() -> TestResult<Self> {
        let clock = ManualClock::new();
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self {
            clock,
            wake,
            client,
            scope,
            _owner: owner,
            directory: Directory::new()?,
            next: 0,
        })
    }
    fn path(&self) -> PathBuf {
        self.directory.0.join("rendezvous")
    }
    /// Start one blocking job that opens its own handle and waits for `kind` until `until`.
    fn waiter(
        &mut self,
        kind: LockKind,
        until: Moment,
        overlapped: bool,
    ) -> TestResult<(OwnedJob<(), ()>, Cell)> {
        self.next += 1;
        let observed = Arc::new(Mutex::new(None));
        let output = observed.clone();
        let path = self.path();
        let job = self.client.submit_blocking(
            &self.scope,
            spec(self.next, JobClass::InfrastructureOrdinary, 0)?,
            move |context| {
                let result = open(&path, overlapped)
                    .map_err(FileLockError::Io)
                    .and_then(|file| lock(&context, file.as_handle(), kind, until));
                *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(result.into());
                Ok::<_, ()>(())
            },
        )?;
        Ok((job, observed))
    }
    /// Wait for the job's terminal outcome, which follows the stored wait result.
    fn settle(&self, job: &mut OwnedJob<(), ()>) -> TestResult<TerminalKind> {
        loop {
            let observed = self.wake.count();
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if let Some(completion) = job.try_take_completion() {
                let completion: JobCompletion<(), ()> = completion;
                return Ok(completion.outcome.kind());
            }
            self.wake.wait_after(observed)?;
        }
    }
}

fn open(path: &Path, overlapped: bool) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(FILE_SHARE_READ_WRITE)
        .custom_flags(if overlapped { FILE_FLAG_OVERLAPPED } else { 0 })
        .open(path)
}

fn moment(millis: u64) -> Moment {
    Moment::from_nanos(millis * 1_000_000)
}

fn observed(cell: &Mutex<Option<Observed>>) -> Option<Observed> {
    *cell.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn hold(path: &Path, kind: LockKind) -> TestResult<File> {
    let holder = open(path, false)?;
    match kind {
        LockKind::Shared => holder.try_lock_shared()?,
        LockKind::Exclusive => holder.try_lock()?,
    }
    Ok(holder)
}

/// Every conflicting pair waits in the kernel and wakes on release without time moving.
#[test]
fn a_conflicting_waiter_wakes_when_the_holder_releases_before_its_deadline() -> TestResult {
    for (held, wanted) in [
        (LockKind::Exclusive, LockKind::Exclusive),
        (LockKind::Exclusive, LockKind::Shared),
        (LockKind::Shared, LockKind::Exclusive),
    ] {
        let mut harness = Harness::new()?;
        let holder = hold(&harness.path(), held)?;
        let until = moment(5_000);
        let (mut job, result) = harness.waiter(wanted, until, true)?;
        harness.clock.wait_for_sleeper(until);
        assert_eq!(observed(&result), None, "{held:?}/{wanted:?} granted while held");
        drop(holder);
        assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
        assert_eq!(observed(&result), Some(Observed::Granted), "{held:?}/{wanted:?}");
        assert_eq!(harness.clock.now(), Moment::from_nanos(0));
    }
    Ok(())
}

/// Shared holders coexist without parking a timer at all.
#[test]
fn shared_locks_are_granted_together_without_waiting() -> TestResult {
    let mut harness = Harness::new()?;
    let _holder = hold(&harness.path(), LockKind::Shared)?;
    let (mut job, result) = harness.waiter(LockKind::Shared, moment(5_000), true)?;
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::Granted));
    assert_eq!(harness.clock.sleepers(), 0);
    Ok(())
}

/// The deadline cancels the kernel request; afterwards the waiter holds nothing.
#[test]
fn a_held_lock_times_out_at_the_deadline_and_leaves_no_lock_behind() -> TestResult {
    for wanted in [LockKind::Shared, LockKind::Exclusive] {
        let mut harness = Harness::new()?;
        let holder = hold(&harness.path(), LockKind::Exclusive)?;
        let until = moment(2_000);
        let (mut job, result) = harness.waiter(wanted, until, true)?;
        harness.clock.wait_for_sleeper(until);
        harness.clock.advance_to(until)?;
        assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
        assert_eq!(observed(&result), Some(Observed::Timeout), "{wanted:?}");
        drop(holder);
        open(&harness.path(), false)?.try_lock()?;
    }
    Ok(())
}

/// Cancellation ends the wait promptly, before the deadline, with no outstanding request.
#[test]
fn cancellation_ends_a_kernel_wait_with_a_typed_cancel_and_no_lock() -> TestResult {
    for wanted in [LockKind::Shared, LockKind::Exclusive] {
        let mut harness = Harness::new()?;
        let holder = hold(&harness.path(), LockKind::Exclusive)?;
        let until = moment(2_000);
        let (mut job, result) = harness.waiter(wanted, until, true)?;
        harness.clock.wait_for_sleeper(until);
        job.cancel();
        assert_eq!(harness.settle(&mut job)?, TerminalKind::Cancelled);
        assert_eq!(observed(&result), Some(Observed::Cancelled), "{wanted:?}");
        assert_eq!(harness.clock.now(), Moment::from_nanos(0));
        drop(holder);
        open(&harness.path(), false)?.try_lock()?;
    }
    Ok(())
}

/// A synchronous handle would block inside LockFileEx without honouring any deadline.
#[test]
fn synchronous_handles_are_rejected_before_any_lock_request() -> TestResult {
    let mut harness = Harness::new()?;
    let (mut job, result) = harness.waiter(LockKind::Exclusive, moment(1_000), false)?;
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::InvalidInput));
    open(&harness.path(), false)?.try_lock()?;
    Ok(())
}

/// Only blocking infrastructure jobs may wait; async jobs are refused without blocking.
#[test]
fn async_jobs_are_refused_without_blocking() -> TestResult {
    let harness = Harness::new()?;
    let path = harness.path();
    let mut job = harness.client.submit_async(
        &harness.scope,
        spec(1, JobClass::IoOrdinary, 0)?,
        move |context| async move {
            let file = open(&path, true).map_err(|_| ())?;
            Ok::<_, ()>(Observed::from(lock(
                &context,
                file.as_handle(),
                LockKind::Shared,
                moment(1),
            )))
        },
    )?;
    let completion = loop {
        let observed = harness.wake.count();
        harness.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        harness.wake.wait_after(observed)?;
    };
    assert!(matches!(
        completion.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(Observed::Unavailable)
    ));
    Ok(())
}

/// A file kept open by a range-lock job after it reports, so its lock stays held.
type Kept = Arc<Mutex<Option<File>>>;

impl Harness {
    /// Start one blocking job that locks `range` exclusively until `until` and keeps its handle.
    fn range_waiter(
        &mut self,
        range: Range,
        until: Moment,
    ) -> TestResult<(OwnedJob<(), ()>, Cell, Kept)> {
        self.next += 1;
        let observed = Arc::new(Mutex::new(None));
        let kept = Arc::new(Mutex::new(None));
        let (output, file_output) = (observed.clone(), kept.clone());
        let path = self.path();
        let job = self.client.submit_blocking(
            &self.scope,
            spec(self.next, JobClass::InfrastructureOrdinary, 0)?,
            move |context| {
                let result = open(&path, true).map_err(FileLockError::Io).and_then(|file| {
                    lock_range(&context, file.as_handle(), LockKind::Exclusive, range, until)?;
                    *file_output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Some(file);
                    Ok(())
                });
                *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(result.into());
                Ok::<_, ()>(())
            },
        )?;
        Ok((job, observed, kept))
    }
}

/// Ranges past the end of an empty file lock independently: a disjoint range is granted at
/// once, the same range waits in the kernel and wakes on release, a bound already reached
/// refuses a held range without waiting, and an empty range is refused.
#[test]
fn byte_ranges_past_the_end_lock_independently_and_wait_only_on_overlap() -> TestResult {
    let mut harness = Harness::new()?;
    let held = Range { offset: 1 << 40, length: 1 };
    let neighbour = Range { offset: (1 << 40) + 1, length: 1 };
    let now = harness.clock.now();
    let (mut job, result, holder) = harness.range_waiter(held, now)?;
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::Granted));
    let (mut job, result, _neighbour) = harness.range_waiter(neighbour, now)?;
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::Granted));
    let (mut job, result, _) = harness.range_waiter(held, now)?;
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::Timeout));
    let until = moment(5_000);
    let (mut job, result, _waiter) = harness.range_waiter(held, until)?;
    harness.clock.wait_for_sleeper(until);
    assert_eq!(observed(&result), None);
    drop(holder.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take());
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::Granted));
    assert_eq!(harness.clock.now(), Moment::from_nanos(0));
    let (mut job, result, _) = harness.range_waiter(Range { offset: 0, length: 0 }, until)?;
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::InvalidInput));
    Ok(())
}
