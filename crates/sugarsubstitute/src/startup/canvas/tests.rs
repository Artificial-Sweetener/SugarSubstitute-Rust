// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real application canvas alongside another scope of its execution owner.

use super::*;
use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::{Condvar, Mutex},
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionError, ExecutionPort, JobClass, JobCompletion, JobId, JobOutcome,
    JobSpec, Operation, OwnedJob, ScopeId, WaitFuture,
};
use sugarsubstitute_execution::ExecutionOwner;

#[cfg(feature = "native-composition")]
mod surface;

struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *revision += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    fn revision(&self) -> u64 {
        *self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn wait_after(&self, revision: u64) -> Result<(), Box<dyn Error>> {
        let guard = self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (guard, timeout) = self
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |value| *value == revision)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && *guard == revision {
            return Err("Application canvas completion did not arrive".into());
        }
        Ok(())
    }
    fn completion<T, E>(
        &self,
        client: &ExecutionClient,
        job: &mut OwnedJob<T, E>,
    ) -> Result<JobCompletion<T, E>, Box<dyn Error>> {
        loop {
            let revision = self.revision();
            client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            self.wait_after(revision)?;
        }
    }
}

fn spec(id: JobId, bytes: u64) -> JobSpec {
    JobSpec {
        id,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: Some(Moment::from_nanos(30_000_000_000)),
        reservation_bytes: bytes,
        replacement: None,
    }
}

#[test]
fn embedded_canvas_shares_capacity_and_retires_without_stopping_startup_scope()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let clock = Arc::new(TestClock);
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::with_memory_capacity(
        clock.clone(),
        wake.clone(),
        NonZeroU64::new(512 * 1024 * 1024).ok_or("Capacity absent")?,
    )?;
    let client = owner.client();
    let other = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut startup =
        client.submit_blocking(&other, spec(jobs.allocate()?, 256 * 1024 * 1024), |_| {
            Ok::<_, std::io::Error>(())
        })?;
    let startup_result = wake.completion(&client, &mut startup)?;
    assert!(matches!(startup_result.outcome, JobOutcome::Success(())));

    let mut canvas = EmbeddedCanvas::new(
        directory.path().to_path_buf(),
        DiagnosticHealth::fixture(),
        clock,
        jobs.clone(),
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
    );
    canvas.start(client.clone(), "en", crate::native_canvas::gpu_policy::EmbeddedGpuPolicy::Cpu)?;
    loop {
        let revision = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        canvas.poll()?;
        if canvas.reported_ready {
            break;
        }
        wake.wait_after(revision)?;
    }
    let session = canvas.session.as_mut().ok_or("Canvas session absent")?;
    assert!(session.frame().is_none());
    assert!(!session.take_picker_request());
    canvas.start(client.clone(), "en", crate::native_canvas::gpu_policy::EmbeddedGpuPolicy::Cpu)?;

    let mut import =
        client.submit_blocking(&other, spec(jobs.allocate()?, 48 * 1024 * 1024), |_| {
            Ok::<_, std::io::Error>(())
        })?;
    let import_result = wake.completion(&client, &mut import)?;
    assert!(matches!(import_result.outcome, JobOutcome::Success(())));
    let cutoff = Moment::from_nanos(15_000_000_000);
    canvas.close_until(cutoff)?;
    canvas.close_until(Moment::from_nanos(20_000_000_000))?;
    assert_eq!(canvas.cutoff, Some(cutoff));
    loop {
        let revision = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if canvas.settled()? {
            break;
        }
        wake.wait_after(revision)?;
    }
    canvas.finish()?;
    assert!(!other.snapshot().cancellation_requested);
    let mut later = client
        .submit_blocking(&other, spec(jobs.allocate()?, 1024), |_| Ok::<_, std::io::Error>(7))?;
    assert!(matches!(wake.completion(&client, &mut later)?.outcome, JobOutcome::Success(7)));
    drop((startup_result, import_result, startup, import, later, other));
    owner.request_shutdown(cutoff);
    let deadline =
        Instant::now().checked_add(Duration::from_secs(10)).ok_or("Deadline overflow")?;
    loop {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if owner.poll_shutdown()?.settled {
            break;
        }
        if Instant::now() >= deadline {
            return Err("Shared execution did not settle".into());
        }
        std::hint::spin_loop();
    }
    owner.finish_shutdown()?;
    Ok(())
}

#[test]
fn closing_before_start_never_admits_a_new_canvas_scope() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let clock = Arc::new(TestClock);
    let owner = ExecutionOwner::new(clock.clone(), Arc::new(Wake::default()))?;
    let mut canvas = EmbeddedCanvas::new(
        directory.path().to_path_buf(),
        DiagnosticHealth::fixture(),
        clock,
        JobSequence::new(),
        ScopeSpec { id: ScopeId::new(1)?, deadline: None },
    );
    canvas.close_until(Moment::from_nanos(10))?;
    assert!(
        canvas
            .start(owner.client(), "en", crate::native_canvas::gpu_policy::EmbeddedGpuPolicy::Cpu)
            .is_err()
    );
    assert!(canvas.settled()?);
    canvas.finish()?;
    Ok(())
}
