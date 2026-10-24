// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe each local-server job through the real execution adapter and finite clock.
use super::*;
use crate::setup_storage::test_worker::TestResult;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Instant,
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionError, ExecutionPort, JobClass, JobId, JobOutcome, JobSpec, Operation,
    OwnedJob, Scope, ScopeId, ScopeSpec,
};
use sugarsubstitute_execution::http_fixture::Exchange;
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock, http_fixture::Server,
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
        let executor = ExecutionOwner::new(MonotonicClock::new(), wake.clone())?;
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
    /// Observe actual server completion; a client succeeding cannot conceal fixture failure.
    pub(crate) fn finish(mut self) -> TestResult {
        let until = Instant::now() + Duration::from_secs(15);
        let completion = loop {
            let revision = *crate::registry::lock(&self.wake.0);
            self.client
                .drain_completions(NonZeroUsize::new(32).ok_or("Invalid completion bound")?)?;
            if let Some(completion) = self.job.try_take_completion() {
                break completion;
            }
            let remaining = until
                .checked_duration_since(Instant::now())
                .ok_or("HTTP fixture did not settle")?;
            let (_guard, timeout) = self
                .wake
                .1
                .wait_timeout_while(crate::registry::lock(&self.wake.0), remaining, |current| {
                    *current == revision
                })
                .map_err(|_| "Poisoned HTTP fixture wake")?;
            if timeout.timed_out() {
                return Err("HTTP fixture did not settle".into());
            }
        };
        match completion.outcome {
            JobOutcome::Success(()) => Ok(()),
            JobOutcome::Failure(cause) => Err(cause.into()),
            other => Err(format!("Unexpected HTTP fixture outcome: {:?}", other.kind()).into()),
        }
    }
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
    }
}
pub(crate) fn complete(path: &str, body: &[u8]) -> Exchange {
    exchange(path, None, 200, &format!("Content-Length: {}\r\n", body.len()), body)
}
