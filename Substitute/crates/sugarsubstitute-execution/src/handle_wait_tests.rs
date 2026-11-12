// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove object waits report the signalled index, time out and cancel on the job's own clock.
use super::*;
use crate::{
    ExecutionClient, ExecutionOwner,
    clock_fixture::ManualClock,
    runtime_test_support::{TestResult, Wake, spec},
};
use std::{
    num::NonZeroUsize,
    os::windows::io::{AsHandle, FromRawHandle, OwnedHandle},
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::{
    Clock, ExecutionPort, JobClass, OwnedJob, Scope, ScopeId, ScopeSpec, TerminalKind,
};
use windows::Win32::System::Threading::{CreateEventW, SetEvent};

/// Observable wait result; the typed error itself is not `Clone`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Observed {
    Signalled(usize),
    Timeout,
    Cancelled,
    Unavailable,
    Io,
}
impl From<Result<usize, HandleWaitError>> for Observed {
    fn from(result: Result<usize, HandleWaitError>) -> Self {
        match result {
            Ok(index) => Self::Signalled(index),
            Err(HandleWaitError::Timeout) => Self::Timeout,
            Err(HandleWaitError::Cancelled) => Self::Cancelled,
            Err(HandleWaitError::Unavailable) => Self::Unavailable,
            Err(HandleWaitError::Io(_)) => Self::Io,
        }
    }
}
type Cell = Arc<Mutex<Option<Observed>>>;

/// A private unnamed manual-reset event.
fn event() -> TestResult<Arc<OwnedHandle>> {
    // SAFETY: a fresh unnamed event; ownership moves into OwnedHandle immediately.
    let handle = unsafe { CreateEventW(None, true, false, None) }?;
    // SAFETY: the handle was just created and has no other owner.
    Ok(Arc::new(unsafe { OwnedHandle::from_raw_handle(handle.0) }))
}
fn signal(event: &OwnedHandle) -> TestResult {
    use std::os::windows::io::AsRawHandle;
    // SAFETY: the event handle is live for this call.
    unsafe { SetEvent(windows::Win32::Foundation::HANDLE(event.as_raw_handle())) }?;
    Ok(())
}

struct Harness {
    clock: Arc<ManualClock>,
    wake: Arc<Wake>,
    client: ExecutionClient,
    scope: Scope,
    _owner: ExecutionOwner,
}
impl Harness {
    fn new() -> TestResult<Self> {
        let clock = ManualClock::new();
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { clock, wake, client, scope, _owner: owner })
    }
    fn waiter(
        &self,
        events: Vec<Arc<OwnedHandle>>,
        until: Moment,
    ) -> TestResult<(OwnedJob<(), ()>, Cell)> {
        let observed = Arc::new(Mutex::new(None));
        let output = observed.clone();
        let job = self.client.submit_blocking(
            &self.scope,
            spec(1, JobClass::InfrastructureOrdinary, 0)?,
            move |context| {
                let handles = events.iter().map(|event| event.as_handle()).collect::<Vec<_>>();
                let result = wait_any(&context, &handles, until);
                *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(result.into());
                Ok::<_, ()>(())
            },
        )?;
        Ok((job, observed))
    }
    fn settle(&self, job: &mut OwnedJob<(), ()>) -> TestResult<TerminalKind> {
        loop {
            let observed = self.wake.count();
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion.outcome.kind());
            }
            self.wake.wait_after(observed)?;
        }
    }
}
fn moment(millis: u64) -> Moment {
    Moment::from_nanos(millis * 1_000_000)
}
fn observed(cell: &Mutex<Option<Observed>>) -> Option<Observed> {
    *cell.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn an_already_signalled_object_is_reported_without_waiting_and_the_lowest_index_wins() -> TestResult
{
    let harness = Harness::new()?;
    let [first, second] = [event()?, event()?];
    signal(&first)?;
    signal(&second)?;
    let (mut job, result) = harness.waiter(vec![first, second], moment(1_000))?;
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::Signalled(0)));
    Ok(())
}

#[test]
fn a_waiter_wakes_with_the_index_signalled_before_its_deadline() -> TestResult {
    let harness = Harness::new()?;
    let [first, second] = [event()?, event()?];
    let until = moment(5_000);
    let (mut job, result) = harness.waiter(vec![first, second.clone()], until)?;
    harness.clock.wait_for_sleeper(until);
    assert_eq!(observed(&result), None);
    signal(&second)?;
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::Signalled(1)));
    assert_eq!(harness.clock.now(), Moment::from_nanos(0));
    Ok(())
}

#[test]
fn an_unsignalled_wait_times_out_at_its_deadline() -> TestResult {
    let harness = Harness::new()?;
    let until = moment(2_000);
    let (mut job, result) = harness.waiter(vec![event()?], until)?;
    harness.clock.wait_for_sleeper(until);
    harness.clock.advance_to(until)?;
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::Timeout));
    Ok(())
}

#[test]
fn cancellation_ends_an_object_wait_before_its_deadline() -> TestResult {
    let harness = Harness::new()?;
    let until = moment(2_000);
    let (mut job, result) = harness.waiter(vec![event()?], until)?;
    harness.clock.wait_for_sleeper(until);
    job.cancel();
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Cancelled);
    assert_eq!(observed(&result), Some(Observed::Cancelled));
    Ok(())
}

#[test]
fn empty_lists_and_async_jobs_are_refused_without_blocking() -> TestResult {
    let harness = Harness::new()?;
    let (mut job, result) = harness.waiter(Vec::new(), moment(1_000))?;
    assert_eq!(harness.settle(&mut job)?, TerminalKind::Success);
    assert_eq!(observed(&result), Some(Observed::Io));
    let pending = event()?;
    let mut job = harness.client.submit_async(
        &harness.scope,
        spec(2, JobClass::IoOrdinary, 0)?,
        move |context| async move {
            Ok::<_, ()>(Observed::from(wait_any(&context, &[pending.as_handle()], moment(1))))
        },
    )?;
    let completion = loop {
        let count = harness.wake.count();
        harness.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        harness.wake.wait_after(count)?;
    };
    assert!(matches!(
        completion.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Success(Observed::Unavailable)
    ));
    Ok(())
}
