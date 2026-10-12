// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise production lifetimes through real execution and injected filesystem effects.
use super::super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{
        Condvar,
        atomic::{AtomicBool, AtomicU64, AtomicUsize},
    },
    time::Duration,
};
use sugarsubstitute_contracts::{
    execution::{
        Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobContext, Moment,
        Operation, ScopeId, ScopeSpec, WaitFuture,
    },
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration},
};
use sugarsubstitute_execution::ExecutionOwner;
pub(super) type TestResult = Result<(), Box<dyn Error>>;
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
struct Wake {
    count: Mutex<usize>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.count) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    fn wait(&self, prior: usize) -> TestResult {
        let guard = lock(&self.count);
        let (_guard, timed) = self
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |count| *count == prior)
            .map_err(|_| "Profile wake poisoned")?;
        if timed.timed_out() {
            return Err("Profile owned completion did not arrive".into());
        }
        Ok(())
    }
}
#[derive(Default)]
pub(super) struct Fake {
    pub(super) acquires: AtomicUsize,
    pub(super) releases: AtomicUsize,
    pub(super) fail_acquire: AtomicBool,
    pub(super) fail_environment: AtomicBool,
    pub(super) fail_release: AtomicBool,
    pub(super) late_acquire: Mutex<Option<Arc<TestClock>>>,
}
impl storage::ProfilePort for Fake {
    fn acquire(
        &self,
        state: &mut storage::NativeState,
        _: &std::path::Path,
        run: RunKey,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        storage::check(context)?;
        self.acquires.fetch_add(1, Ordering::AcqRel);
        state.record = Some(record::Record {
            account: [3; 32],
            folder: [4; 24],
            run,
            state: record::State::Held,
        });
        state.memory = Some(Arc::new(
            context
                .memory()
                .claim(512 * 1024)
                .map_err(|e| ClaimError::new("fake_profile_memory", e))?,
        ));
        state.folder = Some(vec![69, 58, 92, 112]);
        state.held = true;
        state.retired = false;
        if let Some(clock) = lock(&self.late_acquire).take() {
            clock.0.store(100, Ordering::Release);
        }
        if self.fail_acquire.load(Ordering::Acquire) {
            Err(ClaimError::invalid("fake_partial_acquire"))
        } else {
            Ok(())
        }
    }
    fn validate_environment(
        &self,
        _: &mut storage::NativeState,
        observed: &[u16],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        storage::check(context)?;
        if self.fail_environment.load(Ordering::Acquire) || observed != [69, 58, 92, 112] {
            Err(ClaimError::invalid("fake_environment_identity"))
        } else {
            Ok(())
        }
    }
    fn release(
        &self,
        state: &mut storage::NativeState,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        storage::check(context)?;
        self.releases.fetch_add(1, Ordering::AcqRel);
        if self.fail_release.load(Ordering::Acquire) {
            return Err(ClaimError::invalid("fake_close_failure"));
        }
        state.held = false;
        state.retired = true;
        state.release_published = true;
        Ok(())
    }
}
pub(super) struct Harness {
    pub(super) lease: WindowsProfileLease,
    pub(super) registry: WindowsProcessRegistry,
    executor: Option<ExecutionOwner>,
    pub(super) client: ExecutionClient,
    pub(super) scope: Scope,
    pub(super) clock: Arc<TestClock>,
    wake: Arc<Wake>,
    pub(super) fake: Arc<Fake>,
    next: u64,
}
impl Harness {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let wake = Arc::new(Wake::default());
        let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = executor.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let registry = WindowsProcessRegistry::new(run()?.owner)?;
        let mut lease =
            WindowsProfileLease::new(run()?, PathBuf::from(r"E:\fixture\profile"), &registry)?;
        let fake = Arc::new(Fake::default());
        lease.port = fake.clone();
        Ok(Self {
            lease,
            registry,
            executor: Some(executor),
            client,
            scope,
            clock,
            wake,
            fake,
            next: 1,
        })
    }
    pub(super) fn spec(&mut self, deadline: u64) -> Result<JobSpec, Box<dyn Error>> {
        let id = JobId::new(self.next)?;
        self.next += 1;
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(deadline)),
            reservation_bytes: PROFILE_OPERATION_BYTES,
            replacement: None,
        })
    }
    pub(super) fn completion(&mut self) -> Result<ProfileCompletion, Box<dyn Error>> {
        loop {
            let count = *lock(&self.wake.count);
            let batch =
                self.client.drain_completions(NonZeroUsize::new(32).ok_or("Invalid drain")?)?;
            if let Some(completion) = self.lease.consume(&batch)? {
                return Ok(completion);
            }
            self.wake.wait(count)?;
        }
    }
    pub(super) fn acquire(&mut self) -> Result<ProfileCompletion, Box<dyn Error>> {
        let spec = self.spec(100)?;
        self.lease.submit_acquire(&self.client, &self.scope, spec)?;
        let completion = self.completion()?;
        if completion.kind == TerminalKind::Success {
            self.lease.begin_main_attempt(main(1)?)?;
        }
        Ok(completion)
    }
    pub(super) fn folder(&mut self) -> TestResult {
        let spec = self.spec(100)?;
        self.lease.submit_folder_preparation(&self.client, &self.scope, spec, main(1)?)?;
        assert_eq!(self.completion()?.kind, TerminalKind::Success);
        Ok(())
    }
    pub(super) fn release(&mut self, deadline: u64) -> Result<ProfileCompletion, Box<dyn Error>> {
        let spec = self.spec(deadline)?;
        self.lease.submit_release(&self.client, &self.scope, spec, &self.registry)?;
        self.completion()
    }
    pub(super) fn release_on_activity(
        &mut self,
        deadline: u64,
    ) -> Result<ProfileCompletion, Box<dyn Error>> {
        loop {
            let prior = *lock(&self.wake.count);
            let spec = self.spec(deadline)?;
            match self.lease.submit_release(&self.client, &self.scope, spec, &self.registry) {
                Ok(_) => return self.completion(),
                Err(ClaimAdmissionError::Execution(
                    sugarsubstitute_contracts::execution::AdmissionError::MemorySaturated {
                        ..
                    },
                )) => {
                    self.client.drain_completions(NonZeroUsize::new(32).ok_or("zero drain")?)?;
                    self.wake.wait(prior)?;
                }
                Err(cause) => return Err(cause.into()),
            }
        }
    }
    pub(super) fn finish_workers(&mut self) {
        assert!(self.lease.pending.is_none());
        drop(self.executor.take());
    }
}
pub(super) fn main(
    value: u64,
) -> Result<sugarsubstitute_contracts::startup::MainIdentity, Box<dyn Error>> {
    Ok(sugarsubstitute_contracts::startup::MainIdentity {
        run: run()?,
        launch: sugarsubstitute_contracts::startup::LaunchId::new(value)?,
        navigation: sugarsubstitute_contracts::execution::Generation::new(value)?,
    })
}
pub(super) fn run() -> Result<RunKey, Box<dyn Error>> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    })
}
pub(super) fn input() -> Result<crate::WindowsLaunchInput, Box<dyn Error>> {
    let command = crate::WindowsLaunchCommand::from_utf16(
        &r"E:\fixture\host.exe".encode_utf16().collect::<Vec<_>>(),
        &r"E:\fixture".encode_utf16().collect::<Vec<_>>(),
        &[],
    )?;
    Ok(crate::WindowsLaunchInput::new(
        command,
        crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
    )?)
}
