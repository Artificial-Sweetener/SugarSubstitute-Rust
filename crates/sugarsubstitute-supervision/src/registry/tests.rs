// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise production registry state transitions through the real execution owner.
use super::*;
use std::{
    error::Error,
    sync::{
        Condvar,
        atomic::{AtomicBool, AtomicU64, AtomicUsize},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    execution::{
        Clock, CompletionWake, ExecutionError, JobClass, Moment, Operation, ScopeId, ScopeSpec,
        WaitFuture,
    },
    lifecycle::{CoordinatorEpoch, InstanceId, NamespaceId, RunGeneration, RunKey},
};
use sugarsubstitute_execution::ExecutionOwner;
type TestResult = Result<(), Box<dyn Error>>;
struct TestClock(AtomicU64);
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
    fn wait(&self, prior: usize) -> Result<(), Box<dyn Error>> {
        let guard = lock(&self.count);
        let (_guard, timed) = self
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |count| *count == prior)
            .map_err(|_| "Wake poisoned")?;
        if timed.timed_out() {
            return Err("Production completion wake did not arrive".into());
        }
        Ok(())
    }
}
struct Harness {
    registry: WindowsProcessRegistry,
    _executor: ExecutionOwner,
    client: ExecutionClient,
    scope: Scope,
    clock: Arc<TestClock>,
    wake: Arc<Wake>,
    fake: Arc<Fake>,
    next_job: u64,
    next_scope: u64,
}
impl Harness {
    fn new() -> Result<Self, Box<dyn Error>> {
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let wake = Arc::new(Wake::default());
        let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = executor.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let owner = owner()?;
        let fake = Fake::new();
        let mut registry = WindowsProcessRegistry::new(owner)?;
        registry.port = fake.clone();
        Ok(Self {
            registry,
            _executor: executor,
            client,
            scope,
            clock,
            wake,
            fake,
            next_job: 1,
            next_scope: 2,
        })
    }
    fn spec(&mut self, class: JobClass, bytes: u64) -> Result<JobSpec, Box<dyn Error>> {
        let id = JobId::new(self.next_job)?;
        self.next_job += 1;
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class,
            deadline: None,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    fn launch(&mut self, instance: u64) -> Result<LaunchTicket, Box<dyn Error>> {
        let input = input()?;
        let watch = self.spec(JobClass::NativeObservation, 4096)?;
        let creation =
            self.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())?;
        let scope_spec = ScopeSpec { id: ScopeId::new(self.next_scope)?, deadline: None };
        self.next_scope += 1;
        Ok(self.registry.submit_launch(
            &self.client,
            &self.scope,
            scope_spec,
            creation,
            watch,
            binding(instance)?,
            input,
        )?)
    }
    fn drain_until(&mut self, predicate: impl Fn(&WindowsProcessRegistry) -> bool) -> TestResult {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let count = *lock(&self.wake.count);
            self.registry.consume(&self.client.drain_completions(
                std::num::NonZeroUsize::new(32).ok_or("Invalid drain bound")?,
            )?)?;
            if predicate(&self.registry) {
                return Ok(());
            }
            if Instant::now() >= until {
                return Err("Registry did not reach the expected actual outcome".into());
            }
            self.wake.wait(count)?;
        }
    }
    fn created(&mut self, key: ProcessKey) -> TestResult {
        self.drain_until(|registry| {
            registry.snapshot(key).is_ok_and(|snapshot| !snapshot.creation_pending)
                && registry
                    .index(key)
                    .ok()
                    .and_then(|index| registry.records[index].as_ref())
                    .is_some_and(|record| record.root_watch.is_none())
        })
    }
    fn stop(&mut self, key: ProcessKey) -> TestResult {
        self.registry.begin_stop(
            key,
            ShutdownId::new(1)?,
            ShutdownBudget::new(self.clock.now())?,
        )?;
        Ok(())
    }
    fn cleanup(&mut self, key: ProcessKey) -> TestResult {
        let watch = if self.registry.snapshot(key)?.member_watch_required {
            Some(self.spec(JobClass::NativeObservation, 4096)?)
        } else {
            None
        };
        let spec = self.spec(JobClass::InfrastructureLifecycle, 4096)?;
        self.registry.submit_cleanup_step(&self.client, key, &self.scope, spec, watch)?;
        self.drain_until(|registry| {
            registry
                .index(key)
                .ok()
                .and_then(|index| registry.records[index].as_ref())
                .is_some_and(|record| record.cleanup.is_none() && record.member_watch.is_none())
        })
    }
    fn exit(&mut self, key: ProcessKey, identity: Identity, root: bool, code: u32) -> TestResult {
        let index = self.registry.index(key)?;
        let record = self.registry.records[index].as_mut().ok_or("Missing family")?;
        let exit = ProcessExit {
            correlation: sugarsubstitute_execution::windows::NativeCorrelation::new(key.id.get())?,
            process_id: identity.pid,
            creation_time_100ns: identity.birth,
            exit_code: code,
        };
        completion::observe_watch(
            record,
            sugarsubstitute_contracts::execution::JobOutcome::Success(exit),
            Some(identity),
            root,
            record.root_spec.1,
        );
        Ok(())
    }
}
fn owner() -> Result<CoordinatorKey, Box<dyn Error>> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([1; 16])?,
    })
}
fn binding(instance: u64) -> Result<FamilyBinding, Box<dyn Error>> {
    Ok(FamilyBinding::Desktop(RunKey {
        owner: owner()?,
        instance: InstanceId::new(instance)?,
        generation: RunGeneration::new(1)?,
    }))
}
fn input() -> Result<WindowsLaunchInput, Box<dyn Error>> {
    let command = crate::WindowsLaunchCommand::from_utf16(
        &"E:\\fixture\\sugarsubstitute.exe".encode_utf16().collect::<Vec<_>>(),
        &"E:\\fixture".encode_utf16().collect::<Vec<_>>(),
        &[],
    )?;
    Ok(WindowsLaunchInput::new(
        command,
        crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
    )?)
}
mod admission;
mod bounds;
mod claim_identity;
mod cleanup_failure;
mod completions;
mod descendant_facts;
mod enrollment;
mod force;
mod lifecycle;
mod observation_retirement;
mod output;
mod panics;
mod retirement;

mod native_fake;
use native_fake::{Fake, LEAF, ROOT};
