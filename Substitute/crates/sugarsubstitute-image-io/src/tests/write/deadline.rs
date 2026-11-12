// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expire the original admitted job during an interrupted write through its actual injected clock.

use super::*;
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;

struct ManualClock(AtomicU64);
impl Clock for ManualClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.1.notify_all();
        Ok(())
    }
}

struct ExpiringOutput<'a> {
    clock: &'a ManualClock,
    attempts: usize,
}
impl Write for ExpiringOutput<'_> {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.attempts += 1;
        self.clock.0.store(2, Ordering::SeqCst);
        if self.attempts > 1 {
            return Err(io::Error::other("Archive retried after its actual admitted deadline"));
        }
        Err(io::Error::from(io::ErrorKind::Interrupted))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct DeadlineCase {
    clock: Arc<ManualClock>,
    observed: Arc<AtomicBool>,
}
impl CpuTask<(), ImportError> for DeadlineCase {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<()>, ImportError> {
        let mut output = ExpiringOutput { clock: &self.clock, attempts: 0 };
        let result = archive(&mut output, context);
        self.observed.store(
            matches!(
                result,
                Err(ImportError::Image(sugarsubstitute_image_ops::ImageError::Deadline))
            ) && output.attempts == 1
                && context.memory().claim(1024 * 1024).is_ok(),
            Ordering::SeqCst,
        );
        Ok(CpuStep::Complete(()))
    }
}

#[test]
fn archive_interrupted_write_observes_original_deadline_before_another_output_attempt()
-> harness::TestResult {
    let clock = Arc::new(ManualClock(AtomicU64::new(0)));
    let wake = Arc::new(Wake::default());
    let observed = Arc::new(AtomicBool::new(false));
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let worker_observed = observed.clone();
    let mut job = client.submit_cpu(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::ImageEvaluate,
            class: JobClass::CpuDecode,
            deadline: Some(Moment::from_nanos(2)),
            reservation_bytes: 1024 * 1024,
            replacement: None,
        },
        move |_| DeadlineCase { clock, observed: worker_observed },
    )?;
    let outcome = loop {
        let revision = *wake.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(completion) = job.try_take_completion() {
            break completion.outcome;
        }
        let guard = wake.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(
            wake.1
                .wait_while(guard, |current| *current == revision)
                .map_err(|cause| io::Error::other(cause.to_string()))?,
        );
    };
    assert!(matches!(outcome, JobOutcome::TimedOut));
    assert!(observed.load(Ordering::SeqCst));
    Ok(())
}
