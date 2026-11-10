// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Controlled external boundaries and actual central completion observation for relay regressions.
use super::*;
pub(super) use crate::completion_wake::Wake;
use crate::completion_wake::wait_while_watched;
pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
pub(super) struct TestClock(pub(super) AtomicU64);
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
pub(super) struct Gate {
    state: Mutex<(usize, bool)>,
    changed: Condvar,
}
impl Gate {
    pub(super) fn enter(&self) -> TestResult {
        self.arrive();
        let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(wait_while_watched(
            &self.changed,
            state,
            || "the test to release the controlled external blocking boundary".to_owned(),
            |state| !state.1,
        ));
        Ok(())
    }
    pub(super) fn wait_entered(&self, workers: usize) -> TestResult {
        let state = wait_while_watched(
            &self.changed,
            self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
            || format!("{workers} admitted workers to reach the external boundary"),
            |state| state.0 < workers,
        );
        assert_eq!(state.0, workers);
        Ok(())
    }
    pub(super) fn arrive(&self) {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).0 += 1;
        self.changed.notify_all();
    }
    pub(super) fn release(&self) {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).1 = true;
        self.changed.notify_all();
    }
}
pub(super) struct ReleaseOnDrop(pub(super) Arc<Gate>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
pub(super) fn specification(id: JobId, bytes: u64) -> JobSpec {
    JobSpec {
        id,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(60_000_000_000)),
        reservation_bytes: bytes,
        replacement: None,
    }
}
pub(super) fn terminal<T: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, Box<dyn Error + Send + Sync>>,
) -> TestResult<JobCompletion<T, Box<dyn Error + Send + Sync>>> {
    loop {
        let before = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(7))?;
        if let Some(done) = job.try_take_completion() {
            return Ok(done);
        }
        wake.wait_after(before);
    }
}

#[derive(Default)]
pub(super) struct LoanBoundary {
    state: Mutex<(bool, bool, bool)>,
    changed: Condvar,
}
impl LoanBoundary {
    pub(super) fn run(&self, source: ReporterSource, entered: &Gate) -> TestResult {
        entered.arrive();
        let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = wait_while_watched(
            &self.changed,
            state,
            || "the source loan to be requested or the boundary released".to_owned(),
            |state| !state.0 && !state.2,
        );
        if state.2 {
            return Ok(());
        }
        drop(state);
        let loan = source.loan()?;
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.1 = true;
        self.changed.notify_all();
        drop(wait_while_watched(
            &self.changed,
            state,
            || "the test to release the held source loan".to_owned(),
            |state| !state.2,
        ));
        drop(loan);
        Ok(())
    }
    pub(super) fn acquire(&self) -> TestResult {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 = true;
        self.changed.notify_all();
        drop(wait_while_watched(
            &self.changed,
            state,
            || "the worker to acquire the original source loan".to_owned(),
            |state| !state.1,
        ));
        Ok(())
    }
    pub(super) fn release(&self) {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).2 = true;
        self.changed.notify_all();
    }
}
pub(super) struct ReleaseLoanOnDrop(pub(super) Arc<LoanBoundary>);
impl Drop for ReleaseLoanOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
pub(super) fn hold_terminal_credit(
    client: &ExecutionClient,
    wake: &Wake,
    scope: &Scope,
    jobs: &JobSequence,
) -> TestResult<OwnedJob<(), Box<dyn Error + Send + Sync>>> {
    let mut spec = specification(jobs.allocate()?, 0);
    spec.operation = Operation::Readiness;
    spec.class = JobClass::IoLifecycle;
    let job = client.submit_async(scope, spec, |_| async { Ok(()) })?;
    loop {
        let before = wake.revision();
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(7))?;
        if batch.notifications.iter().any(|notice| {
            notice.scope_id == job.scope_id()
                && notice.job_id == job.spec().id
                && notice.kind == TerminalKind::Success
        }) {
            return Ok(job);
        }
        wake.wait_after(before);
    }
}

/// Alter only the external notice; preserve the genuine completion and its original owned payload.
pub(super) fn mismatched_delivery(
    client: &ExecutionClient,
    wake: &Wake,
    relay: &mut ReporterRelay,
    id: JobId,
) -> TestResult {
    loop {
        let before = wake.revision();
        let mut batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(7))?;
        let mut changed = false;
        for notice in &mut batch.notifications {
            if notice.job_id == id {
                assert_eq!(notice.kind, TerminalKind::Success);
                notice.kind = TerminalKind::Failure;
                changed = true;
            }
        }
        if changed {
            assert_eq!(relay.capture(&batch), Err(ReporterSourceIssue::Identity));
            return Ok(());
        }
        relay.capture(&batch)?;
        wake.wait_after(before);
    }
}
