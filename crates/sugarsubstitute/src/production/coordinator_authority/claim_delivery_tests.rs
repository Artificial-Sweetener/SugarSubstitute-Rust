// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose real claims jobs and both actual bounded queues; fake only native filesystem facts.
use super::*;
use std::{
    sync::{Condvar, Mutex},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    configuration::LaunchConfiguration,
    execution::{JobId, JobSpec},
    lifecycle::{CoordinatorEpoch, NamespaceId},
};
use sugarsubstitute_supervision::{
    CLAIM_OPERATION_BYTES, ResolutionTicket,
    claim_boundary_fixture::{self, Control},
};

/// Observe real executor publications without introducing another scheduler or channel.
#[derive(Default)]
struct RevisionWake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for RevisionWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(std::io::Error::other("Claim observer poisoned")),
            )
        })?;
        *revision = revision.checked_add(1).ok_or_else(|| {
            ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(std::io::Error::other("Claim observer exhausted")),
            )
        })?;
        self.changed.notify_all();
        Ok(())
    }
}
/// Retain the actual authority, scopes and claims ahead of the original executor join.
struct Rig {
    authority: CoordinatorAuthority,
    control: Control,
    scope: Scope,
    client: ExecutionClient,
    wake: Arc<RevisionWake>,
    _executor: ExecutionOwner,
    next: u64,
}
impl Drop for Rig {
    /// Release any paused external call before member teardown joins its original worker.
    fn drop(&mut self) {
        self.control.release();
    }
}
impl Rig {
    /// Initialize the genuine claims pool/job; only native filesystem outputs are supplied.
    fn new() -> Result<Self, Box<dyn Error>> {
        let wake = Arc::new(RevisionWake::default());
        let executor = ExecutionOwner::new(Arc::new(TestClock::default()), wake.clone())?;
        let client = executor.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let key = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        };
        let (claims, control) = claim_boundary_fixture::owner(key)?;
        let mut authority =
            CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?)?;
        authority.claims = claims;
        let mut rig =
            Self { authority, control, scope, client, wake, _executor: executor, next: 1 };
        let spec = rig.spec(CLAIM_POOL_BYTES)?;
        rig.authority.claims.submit_initialize(
            &rig.client,
            &rig.scope,
            spec,
            r"E:\native-fake-only".into(),
        )?;
        let batch = rig.wait_batch(spec.id)?;
        rig.authority.claims.consume(&batch)?;
        assert!(
            matches!(rig.authority.claims.take_observation(),Some(ClaimObservation::CoordinatorClaimed(actual)) if actual==key)
        );
        // Select the actual post-startup routing phase, not an elected/authenticated capability.
        // Claims initialization, pool, native boundary and every following event are genuine.
        rig.authority.key = Some(key);
        rig.authority.serving = true;
        Ok(rig)
    }
    /// Use positive original IDs and unchanged bounded finite claims admission.
    fn spec(&mut self, bytes: u64) -> Result<JobSpec, Box<dyn Error>> {
        let id = JobId::new(self.next)?;
        self.next = self.next.checked_add(1).ok_or("Claim fixture ID exhausted")?;
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(1_000_000_000)),
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    /// Admit actual resolution using the original public owner and scope.
    fn resolve(
        &mut self,
        scope: Option<&Scope>,
    ) -> Result<(ResolutionTicket, JobId), Box<dyn Error>> {
        let spec = self.spec(CLAIM_OPERATION_BYTES)?;
        let input = LaunchConfiguration::new(
            r"E:\native-fake-only".into(),
            r"E:\native-fake-only\python.exe".into(),
            vec![],
            8188,
        )?;
        let ticket = self.authority.claims.submit_resolve_launch(
            &self.client,
            scope.unwrap_or(&self.scope),
            spec,
            input,
        )?;
        Ok((ticket, spec.id))
    }
    /// Await the genuine notice while preserving the same immutable complete batch.
    fn wait_batch(&self, id: JobId) -> Result<CompletionBatch, Box<dyn Error>> {
        let until = Instant::now()
            .checked_add(Duration::from_secs(10))
            .ok_or("Observer cutoff overflow")?;
        loop {
            let previous = *self.wake.revision.lock().map_err(|_| "Observer poisoned")?;
            let batch =
                self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            if batch.notifications.iter().any(|notice| notice.job_id == id) {
                return Ok(batch);
            }
            let revision = self.wake.revision.lock().map_err(|_| "Observer poisoned")?;
            let remaining =
                until.checked_duration_since(Instant::now()).ok_or("Actual claim notice absent")?;
            let (_returned, timeout) = self
                .wake
                .changed
                .wait_timeout_while(revision, remaining, |current| *current == previous)
                .map_err(|_| "Observer poisoned")?;
            if timeout.timed_out() {
                return Err("Actual claim notice absent".into());
            }
        }
    }
    /// Invoke the actual composing method with inert unrelated entry input, granting no authority.
    fn consume(&mut self, batch: &CompletionBatch) -> Result<(), Box<dyn Error>> {
        let mut entry = inert_entry(self.authority.original)?;
        assert!(self.authority.consume(
            &mut entry,
            &self.client,
            &self.scope,
            &JobSequence::new(),
            batch
        )?);
        Ok(())
    }
}

/// A real completed resolution reaches the actual serving-phase authority's caller once.
#[test]
fn serving_resolution_observation_reaches_original_caller_once() -> TestResult {
    let mut rig = Rig::new()?;
    let (ticket, id) = rig.resolve(None)?;
    let batch = rig.wait_batch(id)?;
    rig.consume(&batch)?;
    assert!(
        matches!(rig.authority.take_observation(),Some(ClaimObservation::LaunchResolved(actual)) if actual.same_resolution(&ticket))
    );
    assert!(rig.authority.claims.resolved_configuration(&ticket).is_ok());
    assert!(rig.authority.take_observation().is_none());
    rig.consume(&CompletionBatch { notifications: vec![], remaining: 0 })?;
    assert!(rig.authority.take_observation().is_none());
    Ok(())
}

/// Genuine native failure and cancellation keep their original typed producer outcomes.
#[test]
fn serving_failed_and_cancelled_resolution_events_preserve_cause_and_once_delivery() -> TestResult {
    let mut failed = Rig::new()?;
    failed.control.fail_next();
    let (ticket, id) = failed.resolve(None)?;
    let batch = failed.wait_batch(id)?;
    failed.consume(&batch)?;
    let observation = failed.authority.take_observation().ok_or("Native failure stranded")?;
    assert!(
        matches!(observation,ClaimObservation::Failed {target:ClaimTarget::Resolution(actual),cause}
        if actual.same_resolution(&ticket) && failed.control.matches_failure(&cause))
    );
    assert!(failed.authority.take_observation().is_none());
    let mut cancelled = Rig::new()?;
    let scope = cancelled
        .client
        .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(&cancelled.scope))?;
    cancelled.control.pause_next();
    let (ticket, id) = cancelled.resolve(Some(&scope))?;
    cancelled.control.wait_entered()?;
    scope.cancel();
    cancelled.control.release();
    let batch = cancelled.wait_batch(id)?;
    cancelled.consume(&batch)?;
    // Native preparation returned ready, then the same actual job acknowledged cancellation.
    assert!(
        matches!(cancelled.authority.take_observation(),Some(ClaimObservation::LaunchResolved(actual)) if actual.same_resolution(&ticket))
    );
    assert!(
        matches!(cancelled.authority.take_observation(),Some(ClaimObservation::Stopped {target:ClaimTarget::Resolution(actual),kind:TerminalKind::Cancelled}) if actual.same_resolution(&ticket))
    );
    assert!(cancelled.authority.take_observation().is_none());
    Ok(())
}

/// A full retained32 queue leaves the next real producer event owned until a slot returns.
#[test]
fn serving_claim_queue_backpressure_preserves_next_original_event() -> TestResult {
    let mut rig = Rig::new()?;
    let mut tickets = Vec::new();
    for _ in 0..33 {
        let (ticket, id) = rig.resolve(None)?;
        let batch = rig.wait_batch(id)?;
        rig.authority.claims.consume(&batch)?;
        tickets.push(ticket);
    }
    let empty = CompletionBatch { notifications: vec![], remaining: 0 };
    rig.consume(&empty)?;
    assert_eq!(rig.authority.observation_count, 32);
    rig.consume(&empty)?;
    assert_eq!(rig.authority.observation_count, 32);
    for ticket in &tickets[..32] {
        assert!(
            matches!(rig.authority.take_observation(),Some(ClaimObservation::LaunchResolved(actual)) if actual.same_resolution(ticket))
        );
    }
    assert!(rig.authority.take_observation().is_none());
    rig.consume(&empty)?;
    assert!(
        matches!(rig.authority.take_observation(),Some(ClaimObservation::LaunchResolved(actual)) if actual.same_resolution(&tickets[32]))
    );
    assert!(rig.authority.take_observation().is_none());
    Ok(())
}
