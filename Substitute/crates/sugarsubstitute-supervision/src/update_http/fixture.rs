// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe each local-server job through the real execution adapter and a deterministic clock.
use super::*;
use crate::setup_storage::test_worker::TestResult;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionError, ExecutionPort, JobClass, JobId, JobOutcome, JobSpec, Operation,
    OwnedJob, Scope, ScopeId, ScopeSpec,
};
use sugarsubstitute_execution::http_fixture::Exchange;
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, clock_fixture::ManualClock, http_fixture::Server,
};

#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *crate::registry::lock(&self.0) += 1;
        self.1.notify_all();
        Ok(())
    }
}
/// The scoped server remains owned even when an assertion or client operation fails.
pub(crate) struct Running {
    job: OwnedJob<(), io::Error>,
    client: ExecutionClient,
    _scope: Scope,
    wake: Arc<Wake>,
    _executor: ExecutionOwner,
}
impl Running {
    /// Bind before publishing the port, then admit the socket owner with a finite lifetime.
    pub(crate) fn start(
        exchanges: Vec<Exchange>,
        context: &JobContext,
    ) -> Result<(Self, Policy), Box<dyn std::error::Error + Send + Sync>> {
        let server = Server::bind(exchanges, context)?;
        let policy = Policy::loopback(server.port()?)?;
        let wake = Arc::new(Wake::default());
        let executor = ExecutionOwner::new(ManualClock::new(), wake.clone())?;
        let client = executor.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let deadline = client.now()?.checked_add(Duration::from_secs(10))?;
        let job = client.submit_async(
            &scope,
            JobSpec {
                id: JobId::new(1)?,
                operation: Operation::NativeLifecycle,
                class: JobClass::IoLifecycle,
                deadline: Some(deadline),
                reservation_bytes: 1024 * 1024,
                replacement: None,
            },
            move |context| server.run(context),
        )?;
        Ok((Self { job, client, _scope: scope, wake, _executor: executor }, policy))
    }
    /// Observe the server even when its client has already returned an error.
    /// The server's finite job deadline never elapses on the fixture's manual clock, and the
    /// completion wait has no wall-clock bound.
    /// # Errors
    /// Retains the original client and server causes when both operations fail.
    pub(crate) fn finish_after(self, client: TestResult) -> TestResult {
        finish_results(client, self.finish())
    }
    /// Observe actual server completion; a client succeeding cannot conceal fixture failure.
    pub(crate) fn finish(mut self) -> TestResult {
        let completion = loop {
            let revision = *crate::registry::lock(&self.wake.0);
            self.client
                .drain_completions(NonZeroUsize::new(32).ok_or("Invalid completion bound")?)?;
            if let Some(completion) = self.job.try_take_completion() {
                break completion;
            }
            drop(
                self.wake
                    .1
                    .wait_while(crate::registry::lock(&self.wake.0), |current| *current == revision)
                    .map_err(|_| "Poisoned HTTP fixture wake")?,
            );
        };
        match completion.outcome {
            JobOutcome::Success(()) => Ok(()),
            JobOutcome::Failure(cause) => Err(cause.into()),
            other => Err(format!("Unexpected HTTP fixture outcome: {:?}", other.kind()).into()),
        }
    }
}

/// Retain both independently observed failures instead of replacing the client's cause.
#[derive(Debug)]
struct ClientAndServerFailure {
    client: Box<dyn std::error::Error + Send + Sync>,
    server: Box<dyn std::error::Error + Send + Sync>,
}
impl std::fmt::Display for ClientAndServerFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "HTTP fixture client failed: {}; server failed: {}",
            self.client, self.server
        )
    }
}
impl std::error::Error for ClientAndServerFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.client.as_ref())
    }
}

/// Preserve a single original failure or both causes after observing both operations.
fn finish_results(client: TestResult, server: TestResult) -> TestResult {
    match (client, server) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(cause), Ok(())) | (Ok(()), Err(cause)) => Err(cause),
        (Err(client), Err(server)) => Err(Box::new(ClientAndServerFailure { client, server })),
    }
}

#[test]
fn observed_client_and_server_failures_preserve_both_original_causes() -> TestResult {
    assert!(finish_results(Ok(()), Ok(())).is_ok());
    for client_failed in [true, false] {
        let failure: TestResult = Err(io::Error::from_raw_os_error(87).into());
        let result = if client_failed {
            finish_results(failure, Ok(()))
        } else {
            finish_results(Ok(()), failure)
        };
        let cause = result.err().ok_or("Single fixture failure disappeared")?;
        assert_eq!(cause.downcast_ref::<io::Error>().and_then(io::Error::raw_os_error), Some(87));
    }
    let cause = finish_results(
        Err(io::Error::from_raw_os_error(87).into()),
        Err(io::Error::from_raw_os_error(121).into()),
    )
    .err()
    .ok_or("Combined fixture failure disappeared")?;
    let causes = cause.downcast_ref::<ClientAndServerFailure>().ok_or("Combined causes absent")?;
    assert_eq!(
        causes.client.downcast_ref::<io::Error>().and_then(io::Error::raw_os_error),
        Some(87)
    );
    assert_eq!(
        causes.server.downcast_ref::<io::Error>().and_then(io::Error::raw_os_error),
        Some(121)
    );
    let diagnostic = cause.to_string();
    assert!(diagnostic.contains(&causes.client.to_string()));
    assert!(diagnostic.contains(&causes.server.to_string()));
    Ok(())
}
/// Build a raw server response; the body may deliberately be shorter than its declaration.
pub(crate) fn exchange(
    path: &str,
    range: Option<&str>,
    status: u16,
    fields: &str,
    body: &[u8],
) -> Exchange {
    let mut response =
        format!("HTTP/1.1 {status} Fixture\r\nConnection: close\r\n{fields}\r\n").into_bytes();
    response.extend_from_slice(body);
    Exchange {
        path: format!("/{path}"),
        range: range.map(str::to_owned),
        response,
        await_client_close: false,
        cancel_client: None,
    }
}
pub(crate) fn complete(path: &str, body: &[u8]) -> Exchange {
    exchange(path, None, 200, &format!("Content-Length: {}\r\n", body.len()), body)
}
