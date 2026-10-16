// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Controlled external boundaries and actual central completion observation for relay regressions.
use super::*;
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
pub(super) struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    pub(super) fn observed(&self) -> u64 {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub(super) fn wait_after(&self, before: u64) -> TestResult {
        let (count, timeout) = self
            .changed
            .wait_timeout_while(
                self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |count| *count == before,
            )
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && *count == before {
            return Err("Actual original worker completion did not wake".into());
        }
        Ok(())
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
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.1)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && !state.1 {
            return Err("Controlled external blocking boundary did not release".into());
        }
        Ok(())
    }
    pub(super) fn wait_entered(&self, workers: usize) -> TestResult {
        let (state, timeout) = self
            .changed
            .wait_timeout_while(
                self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |state| state.0 < workers,
            )
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && state.0 < workers {
            return Err("Actual admitted workers did not reach external boundary".into());
        }
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
        let before = wake.observed();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(7))?;
        if let Some(done) = job.try_take_completion() {
            return Ok(done);
        }
        wake.wait_after(before)?;
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
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.0 && !state.2)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && !state.0 && !state.2 {
            return Err("Actual source loan was not requested".into());
        }
        if state.2 {
            return Ok(());
        }
        drop(state);
        let loan = source.loan()?;
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.1 = true;
        self.changed.notify_all();
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.2)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && !state.2 {
            return Err("Actual source loan holder did not release".into());
        }
        drop(state);
        drop(loan);
        Ok(())
    }
    pub(super) fn acquire(&self) -> TestResult {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 = true;
        self.changed.notify_all();
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.1)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && !state.1 {
            return Err("Actual worker did not acquire original source loan".into());
        }
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
        let before = wake.observed();
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(7))?;
        if batch.notifications.iter().any(|notice| {
            notice.scope_id == job.scope_id()
                && notice.job_id == job.spec().id
                && notice.kind == TerminalKind::Success
        }) {
            return Ok(job);
        }
        wake.wait_after(before)?;
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
        let before = wake.observed();
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
        wake.wait_after(before)?;
    }
}
