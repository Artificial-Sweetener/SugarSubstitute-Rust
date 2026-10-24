// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Execute real native qualification inside the production admission/lifetime boundary.
use std::{
    io,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionError, ExecutionPort, JobClass, JobCompletion, JobId, JobOutcome,
    JobSpec, Operation, OwnedJob, Scope, ScopeId, ScopeSpec,
};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock,
    http_fixture::{Exchange, Server},
};
pub(super) type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.0) += 1;
        self.1.notify_all();
        Ok(())
    }
}
fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
pub(super) fn run(
    cancelled: bool,
    reservation_bytes: u64,
    body: impl FnOnce(&sugarsubstitute_contracts::execution::JobContext, Arc<Scope>) -> TestResult
    + Send
    + 'static,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(MonotonicClock::new(), wake.clone())?;
    let client = executor.client();
    let scope =
        Arc::new(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
    let worker_scope = scope.clone();
    let observed = Arc::new(Mutex::new(None));
    let worker_observed = observed.clone();
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(client.now()?.checked_add(Duration::from_secs(10))?),
            reservation_bytes,
            replacement: None,
        },
        move |context| {
            *lock(&worker_observed) = Some(body(&context, worker_scope));
            Ok::<_, io::Error>(())
        },
    )?;
    let completion = completion(&mut job, &client, &wake)?;
    lock(&observed).take().ok_or("Native qualification assertions did not finish")??;
    assert!(
        matches!(completion.outcome, JobOutcome::Success(()) if !cancelled)
            || matches!(completion.outcome, JobOutcome::Cancelled if cancelled)
    );
    drop(job);
    drop(scope);
    drop(executor);
    Ok(())
}

/// Observe actual completion through the same adapter wake boundary without timed polling.
fn completion<T, E>(
    job: &mut OwnedJob<T, E>,
    client: &ExecutionClient,
    wake: &Wake,
) -> Result<JobCompletion<T, E>, Box<dyn std::error::Error + Send + Sync>> {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let revision = *lock(&wake.0);
        client.drain_completions(
            std::num::NonZeroUsize::new(32).ok_or("Invalid completion bound")?,
        )?;
        if let Some(completion) = job.try_take_completion() {
            return Ok(completion);
        }
        let remaining = until
            .checked_duration_since(Instant::now())
            .ok_or("Native network qualification did not settle")?;
        let (_guard, status) = wake
            .1
            .wait_timeout_while(lock(&wake.0), remaining, |value| *value == revision)
            .map_err(|_| "Poisoned network qualification wake")?;
        if status.timed_out() {
            return Err("Native network qualification did not settle".into());
        }
    }
}
/// Own an independent finite HTTP fixture through its actual socket-closing terminal outcome.
pub(super) struct Running {
    job: OwnedJob<(), io::Error>,
    client: ExecutionClient,
    wake: Arc<Wake>,
    _scope: Scope,
    _executor: ExecutionOwner,
}
impl Running {
    /// Bind an ephemeral peer which cancels only the supplied client after actual request receipt.
    pub(super) fn cancelling(
        context: &sugarsubstitute_contracts::execution::JobContext,
        client_scope: Arc<Scope>,
    ) -> Result<(Self, u16), Box<dyn std::error::Error + Send + Sync>> {
        let server = Server::bind(
            vec![Exchange {
                path: "/cancel".into(),
                range: None,
                response: b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort".to_vec(),
                await_client_close: true,
                cancel_client: Some(client_scope),
            }],
            context,
        )?;
        let port = server.port()?;
        let wake = Arc::new(Wake::default());
        let executor = ExecutionOwner::new(MonotonicClock::new(), wake.clone())?;
        let client = executor.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let job = client.submit_async(
            &scope,
            JobSpec {
                id: JobId::new(1)?,
                operation: Operation::NativeLifecycle,
                class: JobClass::IoLifecycle,
                deadline: Some(client.now()?.checked_add(Duration::from_secs(10))?),
                reservation_bytes: 1024 * 1024,
                replacement: None,
            },
            move |context| server.run(context),
        )?;
        Ok((Self { job, client, wake, _scope: scope, _executor: executor }, port))
    }
    /// Successful completion proves the real peer observed native client socket retirement.
    pub(super) fn finish(mut self) -> TestResult {
        match completion(&mut self.job, &self.client, &self.wake)?.outcome {
            JobOutcome::Success(()) => Ok(()),
            JobOutcome::Failure(cause) => Err(cause.into()),
            other => Err(format!("Unexpected HTTP fixture outcome: {:?}", other.kind()).into()),
        }
    }
}
