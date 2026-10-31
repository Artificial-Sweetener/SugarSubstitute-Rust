// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare protected cache custody on storage workers and evaluate candidates on CPU workers.

use super::*;
use std::sync::{Condvar, Mutex};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};

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
        *self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    fn completion<T>(
        &self,
        client: &ExecutionClient,
        job: &mut OwnedJob<T, WorkError>,
    ) -> Result<JobCompletion<T, WorkError>, Box<dyn Error>> {
        loop {
            let revision = *self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            let guard = self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let (guard, timeout) = self
                .changed
                .wait_timeout_while(guard, std::time::Duration::from_secs(10), |current| {
                    *current == revision
                })
                .map_err(|cause| std::io::Error::other(cause.to_string()))?;
            if timeout.timed_out() && *guard == revision {
                return Err("Preview task did not reach an observed terminal outcome".into());
            }
        }
    }
}

struct Steps<F> {
    action: F,
    scope: Arc<Scope>,
    resources: Resources,
}
impl<F> CpuTask<(), WorkError> for Steps<F>
where
    F: FnMut(&JobContext, &Scope, &Resources) -> Result<CpuStep<()>, WorkError> + Send + 'static,
{
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<()>, WorkError> {
        (self.action)(context, &self.scope, &self.resources)
    }
}

/// Observe the real terminal outcome after every candidate allocation has been retired by work.
/// Storage retains the original cache/renderer admission; the CPU job separately funds bounded
/// patch work. Its independent scope permits cancellation without cancelling storage retirement.
pub(super) fn run(
    directory: tempfile::TempDir,
    bytes: u64,
    capture: bool,
    cancelled: bool,
    action: impl FnMut(&JobContext, &Scope, &Resources) -> Result<CpuStep<()>, WorkError>
    + Send
    + 'static,
) -> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = owner.client();
    let storage = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let scope =
        Arc::new(client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?);
    let anchor = directory.path().to_path_buf();
    let mut setup = client.submit_blocking(
        &storage,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: bytes,
            replacement: None,
        },
        move |context| {
            let (resources, scene) = initialize(anchor, capture, false, &context)?;
            drop(scene);
            Ok::<_, WorkError>(resources)
        },
    )?;
    let resources = match wake.completion(&client, &mut setup)?.outcome {
        JobOutcome::Success(resources) => resources,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        other => {
            return Err(
                format!("Unexpected preview storage setup outcome: {:?}", other.kind()).into()
            );
        }
    };
    let spec = JobSpec {
        id: JobId::new(2)?,
        operation: Operation::ImageEvaluate,
        class: JobClass::CpuDecode,
        deadline: Some(Moment::from_nanos(30_000_000_000)),
        reservation_bytes: 8 * 1024 * 1024,
        replacement: None,
    };
    let task_scope = scope.clone();
    let mut input = Some(resources.clone());
    let result =
        match client.submit_cpu_owned_input(&scope, spec, &mut input, move |resources, _| Steps {
            action,
            scope: task_scope,
            resources,
        }) {
            Ok(mut job) => match wake.completion(&client, &mut job)?.outcome {
                JobOutcome::Success(()) if !cancelled => Ok(()),
                JobOutcome::Cancelled if cancelled => Ok(()),
                JobOutcome::Failure(cause) => Err(cause.into()),
                other => Err(format!("Unexpected preview task outcome: {:?}", other.kind()).into()),
            },
            Err(cause) => Err(cause.into()),
        };
    let mut retired = Some((resources, input, directory));
    let mut retirement = client.submit_blocking_owned_input(
        &storage,
        JobSpec {
            id: JobId::new(3)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: 64 * 1024,
            replacement: None,
        },
        &mut retired,
        |(resources, rejected, directory), _| {
            drop(resources);
            drop(rejected);
            directory.close()?;
            Ok::<_, WorkError>(())
        },
    )?;
    match wake.completion(&client, &mut retirement)?.outcome {
        JobOutcome::Success(()) => result,
        JobOutcome::Failure(cause) => Err(cause.into()),
        other => {
            Err(format!("Unexpected preview storage retirement outcome: {:?}", other.kind()).into())
        }
    }
}
