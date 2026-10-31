// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise image work on the real executor with an injected deterministic clock.
use std::{
    marker::PhantomData,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_image_ops::ImageError;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct FixedClock;
impl Clock for FixedClock {
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
struct Task<F, T> {
    run: Option<F>,
    output: PhantomData<T>,
}
impl<F, T> CpuTask<T, ImageError> for Task<F, T>
where
    F: FnOnce(&JobContext) -> Result<T, ImageError> + Send + 'static,
    T: Send + 'static,
{
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<T>, ImageError> {
        self.run.take().ok_or(ImageError::Invalid("Test task already finished"))?(context)
            .map(CpuStep::Complete)
    }
}

/// Real execution admission; the condition wait is a bounded external completion boundary.
pub(crate) fn run<T: Send + 'static>(
    bytes: u64,
    action: impl FnOnce(&JobContext) -> Result<T, ImageError> + Send + 'static,
) -> TestResult<T> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::FramePrepare,
        class: JobClass::CpuInteractive,
        deadline: None,
        reservation_bytes: bytes,
        replacement: Some(ReplacementKey {
            surface: SurfaceId::new(1)?,
            revision: Generation::new(1)?,
        }),
    };
    let mut job =
        client.submit_cpu(&scope, spec, |_| Task { run: Some(action), output: PhantomData })?;
    wait(&client, &wake, &mut job)
}

/// Keep the executor alive while a test retains graphics output from its worker.
#[cfg(feature = "native-composition")]
pub(crate) struct BlockingExecution {
    owner: ExecutionOwner,
    client: ExecutionClient,
    scope: Scope,
    wake: Arc<Wake>,
    next_job: u64,
}

#[cfg(feature = "native-composition")]
impl BlockingExecution {
    /// Allocate the next job identity on this owner.
    fn next_spec(&mut self, class: JobClass, bytes: u64) -> TestResult<JobSpec> {
        let id = JobId::new(self.next_job)?;
        self.next_job = self
            .next_job
            .checked_add(1)
            .ok_or_else(|| std::io::Error::other("Test job identity overflow"))?;
        Ok(JobSpec {
            id,
            operation: Operation::Infrastructure,
            class,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: bytes,
            replacement: None,
        })
    }

    /// Run one more blocking graphics operation on this owner and observe its result.
    pub(crate) fn blocking<T, E>(
        &mut self,
        bytes: u64,
        action: impl FnOnce(JobContext) -> Result<T, E> + Send + 'static,
    ) -> TestResult<T>
    where
        T: Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        let spec = self.next_spec(JobClass::InfrastructureOrdinary, bytes)?;
        let mut job = self.client.submit_blocking(&self.scope, spec, action)?;
        wait(&self.client, &self.wake, &mut job)
    }

    /// Run one asynchronous native discovery operation on this owner and observe its result.
    pub(crate) fn asynchronous<T, E, F>(
        &mut self,
        bytes: u64,
        action: impl FnOnce(JobContext) -> F + Send + 'static,
    ) -> TestResult<T>
    where
        T: Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
        F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    {
        let spec = self.next_spec(JobClass::IoOrdinary, bytes)?;
        let mut job = self.client.submit_async(&self.scope, spec, action)?;
        wait(&self.client, &self.wake, &mut job)
    }

    /// Observe an actual DirectComposition wait on the same admitted worker owner.
    pub(crate) fn wait_composition(
        &mut self,
        input: &mut Option<crate::canvas::CompositionWait>,
    ) -> TestResult {
        let id = JobId::new(self.next_job)?;
        self.next_job = self
            .next_job
            .checked_add(1)
            .ok_or_else(|| std::io::Error::other("Test job identity overflow"))?;
        let spec = JobSpec {
            id,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: 64 * 1024,
            replacement: None,
        };
        let mut job = self.client.submit_blocking_owned_input(
            &self.scope,
            spec,
            input,
            |input, context| input.wait(&context),
        )?;
        wait(&self.client, &self.wake, &mut job)
    }

    /// Observe bounded shutdown after the test releases its native output and reservation.
    pub(crate) fn finish(mut self) -> TestResult {
        self.owner.request_shutdown(Moment::from_nanos(10_000_000_000));
        let deadline = std::time::Instant::now()
            .checked_add(Duration::from_secs(10))
            .ok_or_else(|| std::io::Error::other("Execution shutdown deadline overflow"))?;
        loop {
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if self.owner.poll_shutdown()?.settled {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err(std::io::Error::other("Graphics execution did not settle").into());
            }
            std::hint::spin_loop();
        }
        self.owner.finish_shutdown()?;
        Ok(())
    }
}

/// Run actual potentially blocking graphics preparation on an admitted infrastructure worker.
#[cfg(feature = "native-composition")]
pub(crate) fn run_blocking<T, E>(
    bytes: u64,
    action: impl FnOnce(JobContext) -> Result<T, E> + Send + 'static,
) -> TestResult<(T, BlockingExecution)>
where
    T: Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let wake = Arc::new(Wake::default());
    // Later jobs on this owner reserve beside the retained first reservation.
    let capacity = std::num::NonZeroU64::new(bytes.saturating_add(256 * 1024 * 1024))
        .ok_or_else(|| std::io::Error::other("Invalid execution capacity"))?;
    let owner = ExecutionOwner::with_memory_capacity(Arc::new(FixedClock), wake.clone(), capacity)?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: Some(Moment::from_nanos(30_000_000_000)),
        reservation_bytes: bytes,
        replacement: None,
    };
    let mut job = client.submit_blocking(&scope, spec, action)?;
    let output = wait(&client, &wake, &mut job)?;
    Ok((output, BlockingExecution { owner, client, scope, wake, next_job: 2 }))
}

/// Observe the actual terminal result through the same executor used by the test.
fn wait<T, E>(client: &ExecutionClient, wake: &Wake, job: &mut OwnedJob<T, E>) -> TestResult<T>
where
    E: std::error::Error + Send + Sync + 'static,
{
    let outcome = loop {
        let revision = *wake.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        let guard = wake.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (_guard, timeout) = wake
            .changed
            .wait_timeout_while(guard, Duration::from_secs(30), |v| *v == revision)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() {
            return Err(
                std::io::Error::other("Image execution did not reach a terminal outcome").into()
            );
        }
    };
    let maximum =
        NonZeroUsize::new(32).ok_or_else(|| std::io::Error::other("Invalid drain bound"))?;
    client.drain_completions(maximum)?;
    match outcome.outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => Err(Box::new(cause)),
        other => Err(std::io::Error::other(format!(
            "Unexpected image execution outcome: {:?}",
            other.kind()
        ))
        .into()),
    }
}
