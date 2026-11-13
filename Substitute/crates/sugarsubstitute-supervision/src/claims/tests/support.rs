// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Provide deterministic boundary facts while retaining the production execution owner.
use super::*;
use std::{error::Error, time::Duration};
use sugarsubstitute_contracts::{
    execution::{
        Clock, CompletionWake, ExecutionError, JobClass, Moment, Operation, ScopeId, ScopeSpec,
        WaitFuture,
    },
    lifecycle::{CoordinatorEpoch, NamespaceId, ResourceGeneration, ResourceId},
};
use sugarsubstitute_execution::ExecutionOwner;
mod boundary;
mod managed_output_native;
pub(super) use boundary::*;
pub(super) struct Harness {
    pub(super) claims: WindowsResourceClaims,
    pub(super) registry: crate::WindowsProcessRegistry,
    tables: Vec<crate::registry::construction_fixture::TableAdmission>,
    _executor: ExecutionOwner,
    pub(super) client: ExecutionClient,
    pub(super) scope: Scope,
    table_scope: Scope,
    pub(super) fake: Arc<Fake>,
    pub(super) clock: Arc<ClockPort>,
    wake: Arc<Wake>,
    next: u64,
    cleanup_deadline: Option<Moment>,
}
impl Harness {
    /// Construct another actual independent registry without replacing its admitted ledger.
    pub(super) fn construct_registry(
        &mut self,
        owner: sugarsubstitute_contracts::lifecycle::CoordinatorKey,
    ) -> Result<crate::WindowsProcessRegistry, Box<dyn std::error::Error>> {
        let spec = self.spec(crate::WindowsProcessRegistry::required_reservation_bytes())?;
        crate::registry::construction_fixture::construct(
            &self.client,
            &self.table_scope,
            spec,
            owner,
            self.wake.as_ref(),
            &mut self.tables,
            |batch| {
                self.claims.consume(batch)?;
                self.registry.consume(batch)?;
                Ok(())
            },
        )
    }

    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let wake = Arc::new(Wake::default());
        let clock = Arc::new(ClockPort::default());
        let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = executor.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let table_scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        let fake = Arc::new(Fake::default());
        let mut claims = WindowsResourceClaims::new()?;
        claims.port = fake.clone();
        let mut tables = Vec::new();
        let registry = crate::registry::construction_fixture::construct(
            &client,
            &table_scope,
            crate::registry::construction_fixture::first_spec()?,
            owner()?,
            wake.as_ref(),
            &mut tables,
            |_| Ok(()),
        )?;
        Ok(Self {
            claims,
            registry,
            tables,
            _executor: executor,
            client,
            scope,
            table_scope,
            fake,
            clock,
            wake,
            next: 2,
            cleanup_deadline: None,
        })
    }
    pub(super) fn spec(&mut self, bytes: u64) -> Result<JobSpec, Box<dyn Error>> {
        let id = JobId::new(self.next)?;
        self.next += 1;
        Ok(JobSpec {
            id,
            class: JobClass::InfrastructureLifecycle,
            operation: Operation::NativeLifecycle,
            deadline: None,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    /// Latch the original finite cleanup deadline before admission and reuse it after failure.
    pub(super) fn close_spec(&mut self) -> Result<JobSpec, Box<dyn Error>> {
        let until = match self.cleanup_deadline {
            Some(until) => until,
            None => {
                let until = self.client.now()?.checked_add(Duration::from_secs(15))?;
                self.cleanup_deadline = Some(until);
                until
            }
        };
        let mut spec = self.spec(CLAIM_OPERATION_BYTES)?;
        spec.deadline = Some(until);
        Ok(spec)
    }
    pub(super) fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        let spec = self.spec(crate::CLAIM_POOL_BYTES)?;
        self.claims.submit_initialize(
            &self.client,
            &self.scope,
            spec,
            PathBuf::from("E:\\fixture"),
        )?;
        self.drain()?;
        self.claims.bind_process_registry(&self.registry)?;
        Ok(())
    }
    pub(super) fn resolve(&mut self) -> Result<ResolutionTicket, Box<dyn Error>> {
        let spec = self.spec(CLAIM_OPERATION_BYTES)?;
        let ticket =
            self.claims.submit_resolve_launch(&self.client, &self.scope, spec, configuration()?)?;
        self.drain()?;
        Ok(ticket)
    }
    pub(super) fn retire(&mut self, ticket: &ResolutionTicket) -> Result<(), Box<dyn Error>> {
        let spec = self.spec(CLAIM_OPERATION_BYTES)?;
        self.claims.submit_retire_resolution(&self.client, &self.scope, spec, ticket)?;
        self.drain()
    }
    /// Observe typed closure and the exact surviving reservation, including failed closure.
    pub(super) fn close(&mut self, expected_retained_bytes: u64) -> Result<(), Box<dyn Error>> {
        self.claims.close_admission();
        let spec = self.close_spec()?;
        if self.claims.owner.is_some() && self.claims.registry.is_none() {
            self.claims.bind_process_registry(&self.registry)?;
        }
        self.claims.submit_close(&self.client, &self.scope, spec, &mut self.registry)?;
        // Typed completion precedes the blocking worker's final context drop.
        // Its retained result keeps the transient reservation charged while the
        // capacity edge is armed for the exact expected surviving storage.
        self.arm_memory_release(expected_retained_bytes)?;
        self.drain()?;
        self.wait_reserved(&self.scope, expected_retained_bytes)
    }
    /// Arm the real capacity threshold before the held result releases its guard.
    /// Saturation must prove rejection; this probe never supplies a fake charge.
    pub(super) fn arm_memory_release(
        &mut self,
        expected_retained_bytes: u64,
    ) -> Result<(), Box<dyn Error>> {
        let requested_bytes = (256_u64 * 1024 * 1024)
            .checked_sub(expected_retained_bytes)
            .and_then(|bytes| {
                self.tables
                    .iter()
                    .try_fold(bytes, |remaining, table| remaining.checked_sub(table.memory.bytes()))
            })
            .filter(|bytes| *bytes != 0)
            .ok_or("Expected retained storage exceeds the memory-release fixture bound")?;
        let spec = self.spec(requested_bytes)?;
        if !matches!(
            self.client.submit_blocking(&self.scope, spec, |_| Ok::<_, ClaimError>(())),
            Err(sugarsubstitute_contracts::execution::AdmissionError::MemorySaturated {
                requested_bytes: requested, available_bytes,
            }) if requested == requested_bytes && available_bytes < requested_bytes
        ) {
            return Err(
                "Memory-release fixture was not saturated by its retained real guard".into()
            );
        }
        Ok(())
    }
    pub(super) fn completion<T, E>(
        &mut self,
        job: &mut OwnedJob<T, E>,
    ) -> Result<sugarsubstitute_contracts::execution::JobCompletion<T, E>, Box<dyn Error>> {
        loop {
            let revision = *lock(&self.wake.revision);
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            let batch = self
                .client
                .drain_completions(std::num::NonZeroUsize::new(32).ok_or("Invalid bound")?)?;
            self.claims.consume(&batch)?;
            if let Some(completion) = job.try_take_completion() {
                return Ok(completion);
            }
            let guard = lock(&self.wake.revision);
            drop(
                self.wake
                    .changed
                    .wait_while(guard, |current| *current == revision)
                    .map_err(|_| "Wake poisoned")?,
            );
        }
    }
    /// Await exact scope bytes through capacity notifications.
    pub(super) fn wait_reserved(
        &self,
        scope: &Scope,
        expected_bytes: u64,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            let revision = *lock(&self.wake.revision);
            if scope.snapshot().reserved_bytes == expected_bytes {
                return Ok(());
            }
            // Clear a delivered mailbox edge before awaiting the actual retained-byte release.
            self.client
                .drain_completions(std::num::NonZeroUsize::new(32).ok_or("Invalid bound")?)?;
            if scope.snapshot().reserved_bytes == expected_bytes {
                return Ok(());
            }
            let guard = lock(&self.wake.revision);
            drop(
                self.wake
                    .changed
                    .wait_while(guard, |current| *current == revision)
                    .map_err(|_| "Wake poisoned")?,
            );
        }
    }
    pub(super) fn resource(&self, value: u64) -> Result<ResourceKey, Box<dyn Error>> {
        Ok(ResourceKey {
            owner: owner()?,
            id: ResourceId::new(value)?,
            generation: ResourceGeneration::new(1)?,
        })
    }
    pub(super) fn fresh_scope(&mut self) -> Result<Scope, Box<dyn Error>> {
        let scope =
            self.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
        Ok(std::mem::replace(&mut self.scope, scope))
    }
    pub(super) fn drain(&mut self) -> Result<(), Box<dyn Error>> {
        while self.claims.pending.iter().any(Option::is_some) {
            let revision = *lock(&self.wake.revision);
            self.claims.consume(
                &self
                    .client
                    .drain_completions(std::num::NonZeroUsize::new(32).ok_or("Invalid bound")?)?,
            )?;
            if self.claims.pending.iter().all(Option::is_none) {
                break;
            }
            let guard = lock(&self.wake.revision);
            drop(
                self.wake
                    .changed
                    .wait_while(guard, |current| *current == revision)
                    .map_err(|_| "Wake poisoned")?,
            );
        }
        Ok(())
    }
    pub(super) fn drain_registry(&mut self) -> Result<(), Box<dyn Error>> {
        while !self.registry.is_empty() {
            let revision = *lock(&self.wake.revision);
            let batch = self
                .client
                .drain_completions(std::num::NonZeroUsize::new(32).ok_or("Invalid bound")?)?;
            self.claims.consume(&batch)?;
            self.registry.consume(&batch)?;
            if self.registry.is_empty() {
                break;
            }
            let guard = lock(&self.wake.revision);
            drop(
                self.wake
                    .changed
                    .wait_while(guard, |current| *current == revision)
                    .map_err(|_| "Wake poisoned")?,
            );
        }
        Ok(())
    }
}
pub(super) fn owner() -> Result<CoordinatorKey, ClaimError> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([1; 16])
            .map_err(|cause| ClaimError::new("fixture_epoch", cause))?,
    })
}
pub(super) fn configuration() -> Result<LaunchConfiguration, Box<dyn Error>> {
    Ok(LaunchConfiguration::new(
        PathBuf::from("E:\\fixture"),
        PathBuf::from("E:\\fixture\\python.exe"),
        vec!["".into(), "漢字\\\"".into()],
        8188,
    )?)
}
