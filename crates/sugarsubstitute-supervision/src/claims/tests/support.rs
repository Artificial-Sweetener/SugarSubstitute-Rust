// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Provide deterministic boundary facts while retaining the production execution owner.
use super::*;
use std::{
    error::Error,
    time::{Duration, Instant},
};
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
    _executor: ExecutionOwner,
    pub(super) client: ExecutionClient,
    pub(super) scope: Scope,
    pub(super) fake: Arc<Fake>,
    wake: Arc<Wake>,
    next: u64,
}
impl Harness {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let wake = Arc::new(Wake::default());
        let executor = ExecutionOwner::new(Arc::new(ClockPort::default()), wake.clone())?;
        let client = executor.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let fake = Arc::new(Fake::default());
        let mut claims = WindowsResourceClaims::new()?;
        claims.port = fake.clone();
        Ok(Self {
            claims,
            registry: crate::WindowsProcessRegistry::new(owner()?)?,
            _executor: executor,
            client,
            scope,
            fake,
            wake,
            next: 1,
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
        let spec = self.spec(CLAIM_OPERATION_BYTES)?;
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
        let deadline = Instant::now() + Duration::from_secs(10);
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
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Fixture typed completion deadline expired".into());
            }
            let guard = lock(&self.wake.revision);
            let (_guard, timeout) = self
                .wake
                .changed
                .wait_timeout_while(guard, remaining, |current| *current == revision)
                .map_err(|_| "Wake poisoned")?;
            if timeout.timed_out() {
                return Err("Fixture typed completion wake deadline expired".into());
            }
        }
    }
    /// Await exact scope bytes through capacity notifications within the original ten-second guard.
    pub(super) fn wait_reserved(
        &self,
        scope: &Scope,
        expected_bytes: u64,
    ) -> Result<(), Box<dyn Error>> {
        let deadline = Instant::now() + Duration::from_secs(10);
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
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Actual memory release deadline expired".into());
            }
            let guard = lock(&self.wake.revision);
            let (_guard, timeout) = self
                .wake
                .changed
                .wait_timeout_while(guard, remaining, |current| *current == revision)
                .map_err(|_| "Wake poisoned")?;
            if timeout.timed_out() {
                return Err("Actual memory release wake deadline expired".into());
            }
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
            self.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        Ok(std::mem::replace(&mut self.scope, scope))
    }
    pub(super) fn drain(&mut self) -> Result<(), Box<dyn Error>> {
        let deadline = Instant::now() + Duration::from_secs(10);
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
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Claim completion deadline expired".into());
            }
            let guard = lock(&self.wake.revision);
            let (_guard, timeout) = self
                .wake
                .changed
                .wait_timeout_while(guard, remaining, |current| *current == revision)
                .map_err(|_| "Wake poisoned")?;
            if timeout.timed_out() {
                return Err("Claim completion wake deadline expired".into());
            }
        }
        Ok(())
    }
    pub(super) fn drain_registry(&mut self) -> Result<(), Box<dyn Error>> {
        let deadline = Instant::now() + Duration::from_secs(10);
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
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Registry retirement deadline expired".into());
            }
            let guard = lock(&self.wake.revision);
            let (_guard, timeout) = self
                .wake
                .changed
                .wait_timeout_while(guard, remaining, |current| *current == revision)
                .map_err(|_| "Wake poisoned")?;
            if timeout.timed_out() {
                return Err("Registry retirement wake deadline expired".into());
            }
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
