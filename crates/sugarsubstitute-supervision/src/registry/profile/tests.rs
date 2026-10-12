// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove retained family identity and current inventory through a fake native boundary.
use super::*;
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
        Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobOutcome, Moment,
        Operation, ScopeId, ScopeSpec, WaitFuture,
    },
    lifecycle::{
        CoordinatorEpoch, InstanceId, NamespaceId, RunGeneration, ShutdownBudget, ShutdownId,
    },
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
    fn wait(&self, prior: usize) -> TestResult {
        let guard = lock(&self.count);
        let (_guard, timed) = self
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |count| *count == prior)
            .map_err(|_| "Family wake poisoned")?;
        if timed.timed_out() {
            return Err("Family completion missing".into());
        }
        Ok(())
    }
}
struct Fake {
    inventories: AtomicUsize,
    current: AtomicUsize,
    incomplete: AtomicBool,
    late: AtomicBool,
    clock: Arc<TestClock>,
}
impl NativePort for Fake {
    fn create(
        &self,
        family: &mut Family,
        _: &mut WindowsLaunchInput,
    ) -> Result<Identity, RegistryError> {
        let identity = Identity { pid: 123, birth: 456 };
        family.job = Some(10);
        family.root = Some(identity);
        family.count = 1;
        family.members[0] =
            Some(native_process::Member { handle: Some(12), identity, exited: false });
        Ok(identity)
    }
    fn resume(&self, family: &mut Family) -> Result<(), RegistryError> {
        family.resumed = true;
        Ok(())
    }
    fn duplicate(&self, _: &mut Family, _: usize) -> Result<WatchInput, RegistryError> {
        Ok(WatchInput::Simulated)
    }
    fn enroll_desktop(
        &self,
        _: &mut Family,
        _: RunKey,
        _: ProcessKey,
    ) -> Result<(), RegistryError> {
        Ok(())
    }
    fn enroll_diagnostic(
        &self,
        _: &mut Family,
        _: sugarsubstitute_contracts::incident::DiagnosticKey,
        _: ProcessKey,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::invalid("profile_fixture_diagnostic_role"))
    }
    fn seal(&self, family: &mut Family) -> Result<(), RegistryError> {
        family.sealed = true;
        Ok(())
    }
    fn inventory(&self, family: &mut Family) -> Result<native_process::Inventory, RegistryError> {
        self.inventories.fetch_add(1, Ordering::AcqRel);
        family.incomplete = self.incomplete.load(Ordering::Acquire);
        let bytes = [1u32.to_ne_bytes(), 1u32.to_ne_bytes()].concat();
        family.query.fill(0);
        for (index, chunk) in bytes.chunks_exact(size_of::<usize>()).enumerate() {
            family.query[index] = usize::from_ne_bytes(std::array::from_fn(|offset| chunk[offset]));
        }
        family.query[8 / size_of::<usize>()] = self.current.load(Ordering::Acquire);
        if self.late.load(Ordering::Acquire) {
            self.clock.0.store(100, Ordering::Release);
        }
        Ok(native_process::Inventory { empty: false, count: 1 })
    }
    fn terminate(&self, family: &mut Family) -> Result<(), RegistryError> {
        family.forced = true;
        Ok(())
    }
    fn close(&self, family: &mut Family) -> Result<(), RegistryError> {
        family.job = None;
        family.thread = None;
        for member in family.members.iter_mut().flatten() {
            member.handle = None;
        }
        Ok(())
    }
}
struct Harness {
    registry: WindowsProcessRegistry,
    _tables: Vec<crate::registry::construction_fixture::TableAdmission>,
    _owner: ExecutionOwner,
    client: ExecutionClient,
    scope: Scope,
    _table_scope: Scope,
    wake: Arc<Wake>,
    fake: Arc<Fake>,
    next: u64,
}
impl Harness {
    fn new() -> Result<Self, Box<dyn Error>> {
        let clock = Arc::new(TestClock(AtomicU64::new(0)));
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let table_scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        let fake = Arc::new(Fake {
            inventories: AtomicUsize::new(0),
            current: AtomicUsize::new(123),
            incomplete: AtomicBool::new(false),
            late: AtomicBool::new(false),
            clock,
        });
        let mut tables = Vec::new();
        let mut registry = crate::registry::construction_fixture::construct(
            &client,
            &table_scope,
            crate::registry::construction_fixture::first_spec()?,
            run()?.owner,
            wake.as_ref(),
            &mut tables,
            |_| Ok(()),
        )?;
        registry.port = fake.clone();
        Ok(Self {
            registry,
            _tables: tables,
            _owner: owner,
            client,
            scope,
            _table_scope: table_scope,
            wake,
            fake,
            next: 2,
        })
    }
    fn spec(
        &mut self,
        class: JobClass,
        bytes: u64,
        deadline: Option<Moment>,
    ) -> Result<JobSpec, Box<dyn Error>> {
        let id = JobId::new(self.next)?;
        self.next += 1;
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class,
            deadline,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    fn launch(&mut self) -> Result<LaunchTicket, Box<dyn Error>> {
        let command = crate::WindowsLaunchCommand::from_utf16(
            &r"E:\fixture\host.exe".encode_utf16().collect::<Vec<_>>(),
            &r"E:\fixture".encode_utf16().collect::<Vec<_>>(),
            &[],
        )?;
        let input = WindowsLaunchInput::new(
            command,
            crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
        )?;
        let watch = self.spec(JobClass::NativeObservation, 4096, None)?;
        let create =
            self.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes(), None)?;
        let ticket = self.registry.submit_launch(
            &self.client,
            &self.scope,
            ScopeSpec { id: ScopeId::new(3)?, deadline: None },
            create,
            watch,
            FamilyBinding::Desktop(run()?),
            input,
        )?;
        loop {
            let count = *lock(&self.wake.count);
            self.registry
                .consume(&self.client.drain_completions(NonZeroUsize::new(32).ok_or("Drain")?)?)?;
            let snapshot = self.registry.snapshot(ticket.process())?;
            if snapshot.started && !snapshot.creation_pending {
                return Ok(ticket);
            }
            self.wake.wait(count)?;
        }
    }
    fn qualify(
        &mut self,
        proof: ProfileFamily,
        pid: u32,
    ) -> Result<JobOutcome<Identity, RegistryError>, Box<dyn Error>> {
        let spec =
            self.spec(JobClass::InfrastructureLifecycle, 4096, Some(Moment::from_nanos(100)))?;
        let mut job = self.client.submit_blocking(&self.scope, spec, move |context| {
            proof.qualify_browser(pid, &context)
        })?;
        loop {
            let count = *lock(&self.wake.count);
            self.registry
                .consume(&self.client.drain_completions(NonZeroUsize::new(32).ok_or("Drain")?)?)?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion.outcome);
            }
            self.wake.wait(count)?;
        }
    }
}
fn run() -> Result<RunKey, Box<dyn Error>> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    })
}
#[test]
fn browser_membership_uses_fresh_complete_inventory_and_kernel_birth() -> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch()?;
    let proof = h.registry.profile_family(ticket, run()?)?;
    assert!(proof.is_live());
    assert!(matches!(
        h.qualify(proof, 123)?,
        JobOutcome::Success(Identity { pid: 123, birth: 456 })
    ));
    assert_eq!(h.fake.inventories.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn older_retained_member_and_incomplete_query_cannot_authorize() -> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch()?;
    h.fake.current.store(124, Ordering::Release);
    let proof = h.registry.profile_family(ticket, run()?)?;
    assert!(matches!(h.qualify(proof, 123)?, JobOutcome::Failure(_)));
    h.fake.current.store(123, Ordering::Release);
    h.fake.incomplete.store(true, Ordering::Release);
    let proof = h.registry.profile_family(ticket, run()?)?;
    assert!(matches!(h.qualify(proof, 123)?, JobOutcome::Failure(_)));
    Ok(())
}
#[test]
fn stopping_family_invalidates_retained_proof_before_native_qualification() -> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch()?;
    let proof = h.registry.profile_family(ticket, run()?)?;
    h.registry.begin_stop(
        ticket.process(),
        ShutdownId::new(1)?,
        ShutdownBudget::new(Moment::from_nanos(0))?,
    )?;
    assert!(!proof.is_live());
    assert!(matches!(h.qualify(proof, 123)?, JobOutcome::Failure(_)));
    assert_eq!(h.fake.inventories.load(Ordering::Acquire), 0);
    Ok(())
}
#[test]
fn qualification_rechecks_original_deadline_after_actual_inventory_return() -> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch()?;
    h.fake.late.store(true, Ordering::Release);
    let proof = h.registry.profile_family(ticket, run()?)?;
    assert!(matches!(h.qualify(proof, 123)?, JobOutcome::TimedOut));
    assert_eq!(h.fake.inventories.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn changed_run_binding_cannot_create_an_opaque_desktop_family() -> TestResult {
    let mut h = Harness::new()?;
    let ticket = h.launch()?;
    let mut foreign = run()?;
    foreign.generation = RunGeneration::new(2)?;
    assert!(h.registry.profile_family(ticket, foreign).is_err());
    assert_eq!(h.fake.inventories.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn matching_public_facts_do_not_allow_a_foreign_executor_to_query_membership() -> TestResult {
    let mut original = Harness::new()?;
    let ticket = original.launch()?;
    let proof = original.registry.profile_family(ticket, run()?)?;
    let mut foreign = Harness::new()?;
    assert!(matches!(foreign.qualify(proof, 123)?, JobOutcome::Failure(_)));
    assert_eq!(original.fake.inventories.load(Ordering::Acquire), 0);
    Ok(())
}

impl crate::registry::construction_fixture::ConstructionWake for Wake {
    fn revision(&self) -> u64 {
        *crate::registry::lock(&self.count) as u64
    }
    fn wait_revision(&self, previous: u64) -> Result<(), Box<dyn std::error::Error>> {
        let guard = crate::registry::lock(&self.count);
        let (_guard, timed) = self
            .changed
            .wait_timeout_while(guard, std::time::Duration::from_secs(10), |value| {
                *value as u64 == previous
            })
            .map_err(|_| "Constructor completion wake poisoned")?;
        if timed.timed_out() {
            Err("Actual constructor completion did not arrive".into())
        } else {
            Ok(())
        }
    }
}
