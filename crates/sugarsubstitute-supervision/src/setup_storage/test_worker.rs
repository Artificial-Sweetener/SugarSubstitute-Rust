// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deterministic operation clock and actual scoped storage-job execution for native tests.
use std::{error::Error, io};
use sugarsubstitute_contracts::execution::JobContext;
pub(crate) type TestResult = Result<(), Box<dyn Error + Send + Sync>>;
/// Create an owned native fixture with an empty authoritative selection.
pub(crate) fn installation(
    number: u8,
    context: &JobContext,
) -> Result<tempfile::TempDir, Box<dyn Error + Send + Sync>> {
    use sugarsubstitute_contracts::installation::{InstallationId, InstallationRecord};
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join(super::ownership::MARKER), super::ownership::CONTENT)?;
    super::SelectionStore::initialize(
        root.path(),
        &InstallationRecord::empty(InstallationId::new(format!("{number:032x}"))?),
        context,
    )?;
    Ok(root)
}
/// Inspect one explicitly owned cache fixture generation for deliberate corruption assertions.
/// Runtime consumers retain a `CacheLease`; this test helper grants no custody or authority.
pub(crate) fn cache_file(
    anchor: &std::path::Path,
    namespace: super::cache::Namespace,
    leaf: &str,
) -> io::Result<std::path::PathBuf> {
    let parent = anchor.join(super::cache::DIRECTORY).join(namespace.directory());
    let entries = std::fs::read_dir(parent)?.collect::<io::Result<Vec<_>>>()?;
    if entries.len() != 1 {
        return Err(io::Error::other("Cache fixture must contain exactly one generation"));
    }
    Ok(entries[0].path().join("data").join(leaf))
}
use std::{
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobId, JobOutcome, JobSpec,
    Moment, Operation, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::ExecutionOwner;
pub(crate) mod pipeline;
pub(super) mod trace;

#[derive(Default)]
pub(crate) struct TestClock(
    AtomicU64,
    Mutex<&'static str>,
    OnceLock<Box<dyn Fn() -> bool + Send + Sync>>,
);
impl TestClock {
    /// Observe operation boundaries and expire when an isolated fixture reaches its fault state.
    /// The observer may run on multiple execution-owner threads and must not call this clock.
    pub(crate) fn observe_checks(
        &self,
        observer: impl Fn() -> bool + Send + Sync + 'static,
    ) -> io::Result<()> {
        self.2.set(Box::new(observer)).map_err(|_| io::Error::other("Clock observer already set"))
    }
    /// Label actual fixture progress without advancing its deterministic operation clock.
    pub(crate) fn checkpoint(&self, phase: &'static str) {
        *crate::registry::lock(&self.1) = phase;
    }
    pub(crate) fn expire(&self) {
        self.0.store(2_000_000_000, Ordering::SeqCst);
    }
}
impl Clock for TestClock {
    fn now(&self) -> Moment {
        if self.2.get().is_some_and(|observe| observe()) {
            self.expire();
        }
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
        *crate::registry::lock(&self.0) += 1;
        self.1.notify_all();
        Ok(())
    }
}
pub(super) fn run(
    body: impl FnOnce(&JobContext, &TestClock) -> TestResult + Send + 'static,
) -> TestResult {
    execute(body, false, 16 * 1024 * 1024)
}
pub(super) fn run_expiring(
    body: impl FnOnce(&JobContext, &TestClock) -> TestResult + Send + 'static,
) -> TestResult {
    execute(body, true, 16 * 1024 * 1024)
}
/// Admit a native boundary test with the production owner's explicit workspace reservation.
pub(crate) fn run_reserved(
    reservation_bytes: u64,
    body: impl FnOnce(&JobContext, &TestClock) -> TestResult + Send + 'static,
) -> TestResult {
    execute(body, false, reservation_bytes)
}
/// Observe an expired native operation after its owner has completed mandatory publication.
pub(crate) fn run_reserved_expiring(
    reservation_bytes: u64,
    body: impl FnOnce(&JobContext, &TestClock) -> TestResult + Send + 'static,
) -> TestResult {
    execute(body, true, reservation_bytes)
}
fn execute(
    body: impl FnOnce(&JobContext, &TestClock) -> TestResult + Send + 'static,
    expires: bool,
    reservation_bytes: u64,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock::default());
    let observer_clock = clock.clone();
    let observed = Arc::new(Mutex::new(None));
    let worker_observed = observed.clone();
    let stage = Arc::new(AtomicU64::new(0));
    let worker_stage = stage.clone();
    let trace = trace::StorageTrace::default();
    let worker_trace = trace.clone();
    let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(2_000_000_000)),
            reservation_bytes,
            replacement: None,
        },
        move |context| {
            worker_stage.store(1, Ordering::SeqCst);
            let result = tracing::subscriber::with_default(worker_trace, || body(&context, &clock));
            *crate::registry::lock(&worker_observed) = Some(result);
            worker_stage.store(2, Ordering::SeqCst);
            Ok::<_, io::Error>(())
        },
    )?;
    let until = Instant::now() + Duration::from_secs(20);
    let completion = loop {
        let revision = *crate::registry::lock(&wake.0);
        client.drain_completions(
            std::num::NonZeroUsize::new(32).ok_or("Invalid completion bound")?,
        )?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        let remaining = until.checked_duration_since(Instant::now()).ok_or_else(|| {
            format!(
                "Storage observer expired: stage={}, phase={}, status={:?}, {}",
                stage.load(Ordering::SeqCst),
                *crate::registry::lock(&observer_clock.1),
                job.status(),
                trace.snapshot()
            )
        })?;
        let (_guard, timeout) = wake
            .1
            .wait_timeout_while(crate::registry::lock(&wake.0), remaining, |current| {
                *current == revision
            })
            .map_err(|_| "Poisoned storage completion wake")?;
        if timeout.timed_out() {
            return Err(format!(
                "Storage observer expired: stage={}, phase={}, status={:?}, {}",
                stage.load(Ordering::SeqCst),
                *crate::registry::lock(&observer_clock.1),
                job.status(),
                trace.snapshot()
            )
            .into());
        }
    };
    drop(job);
    drop(executor);
    crate::registry::lock(&observed).take().ok_or("Storage assertions did not complete")??;
    match completion.outcome {
        JobOutcome::Success(()) if !expires => Ok(()),
        JobOutcome::TimedOut if expires => Ok(()),
        JobOutcome::Failure(cause) => Err(cause.into()),
        other => Err(format!("Unexpected storage outcome: {:?}", other.kind()).into()),
    }
}
