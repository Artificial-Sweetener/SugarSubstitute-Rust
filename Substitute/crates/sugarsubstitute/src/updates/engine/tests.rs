// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real execution and rejected native admission verify campaign/cleanup ownership without child creation.
use super::*;
use crate::updates::test_support::{Manual, Wake, observe, untrusted_input};
use std::{num::NonZeroUsize, sync::atomic::Ordering, time::Duration};
use sugarsubstitute_execution::ExecutionOwner;

/// One actual executor and invalid input, isolated from every application or installation lease.
struct Harness {
    engine: Engine,
    owner: ExecutionOwner,
    clock: Arc<Manual>,
    wake: Arc<Wake>,
    jobs: JobSequence,
}
impl Harness {
    /// Only the external input is invalid; the executor, scopes, process registry and cleanup are real.
    fn new() -> Result<Self> {
        let clock = Arc::new(Manual::default());
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let until = clock.now().checked_add(Duration::from_secs(30))?;
        let engine = Engine {
            work: client
                .create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: Some(until) }, None)?,
            cleanup: client
                .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
            state: Arc::new(Mutex::new(State {
                phase: Phase::Input(Some(untrusted_input()?)),
                failure: None,
            })),
            client,
            until,
            job: None,
            parent: None,
            closing: None,
            next_scope: 5,
            succeeded: false,
        };
        Ok(Self { engine, owner, clock, wake, jobs: JobSequence::new() })
    }

    /// Deliver the actual completion batch before advancing dependent ownership.
    fn step(&mut self) -> Result<()> {
        let batch = self.engine.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        self.engine.advance(&batch, &self.jobs)
    }

    /// Observe cleanup and release owned scopes before joining the sole execution owner.
    fn finish(mut self) -> Result<()> {
        self.engine.close(ShutdownBudget::new(self.clock.now())?)?;
        let wake = self.wake.clone();
        observe(&wake, || {
            self.step()?;
            self.engine.settled()
        })?;
        assert!(!self.engine.succeeded());
        let until = self.engine.closing.ok_or("Cleanup absent")?.settle_until;
        let client = self.engine.client.clone();
        drop(self.engine);
        self.owner.request_shutdown(until);
        observe(&wake, || {
            client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            Ok(self.owner.poll_shutdown()?.settled)
        })?;
        self.owner.finish_shutdown()?;
        Ok(())
    }
}

#[test]
fn close_before_input_admission_retires_on_live_cleanup_without_creating_a_process() -> Result<()> {
    let mut h = Harness::new()?;
    let budget = ShutdownBudget::new(h.clock.now())?;
    h.engine.close(budget)?;
    h.engine.work.cancel();
    h.step()?;
    assert!(h.engine.parent.is_none());
    assert_eq!(
        h.engine.job.as_ref().ok_or("Cleanup not admitted")?.spec().deadline,
        Some(budget.settle_until)
    );
    assert!(!h.engine.settled()?);
    h.clock.0.store(1_000_000_000, Ordering::SeqCst);
    h.engine.close(ShutdownBudget::new(h.clock.now())?)?;
    assert_eq!(h.engine.closing, Some(budget));
    h.finish()
}

#[test]
fn expired_campaign_preserves_input_and_does_not_renew_the_next_helper_deadline() -> Result<()> {
    let mut h = Harness::new()?;
    let until = h.engine.until;
    h.clock.0.store(31_000_000_000, Ordering::SeqCst);
    assert!(h.engine.launch().is_err());
    assert_eq!(h.engine.until, until);
    assert!(h.engine.parent.is_none());
    assert!(matches!(h.engine.state()?.phase, Phase::Input(Some(_))));
    h.finish()
}

#[test]
fn rejected_preparation_role_reports_failure_then_retires_without_application_ownership()
-> Result<()> {
    let mut h = Harness::new()?;
    h.step()?;
    let parent = h.engine.parent.as_ref().ok_or("Contained parent absent")?;
    assert!(!parent.borrows_application());
    assert!(!parent.settled());
    let wake = h.wake.clone();
    observe(&wake, || match h.step() {
        Ok(()) => Ok(false),
        Err(cause) => {
            assert!(
                cause
                    .downcast_ref::<sugarsubstitute_supervision::RegistryAdmissionError>()
                    .is_some()
            );
            Ok(true)
        }
    })?;
    assert!(h.engine.parent.is_none());
    assert!(!h.engine.succeeded());
    h.finish()
}

#[test]
fn storage_failure_is_observed_before_retained_input_is_cleaned_up() -> Result<()> {
    let mut h = Harness::new()?;
    h.engine.submit(&h.jobs, 4096)?;
    assert_eq!(
        h.engine.job.as_ref().ok_or("Storage job absent")?.spec().deadline,
        Some(h.engine.until)
    );
    let wake = h.wake.clone();
    observe(&wake, || {
        h.engine.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(h.engine
            .job
            .as_ref()
            .is_some_and(|job| matches!(job.status().phase, JobPhase::Terminal(_))))
    })?;
    h.engine.close(ShutdownBudget::new(h.clock.now())?)?;
    let cause = h.step().err().ok_or("Storage failure was discarded on close")?;
    assert!(cause.to_string().contains("Engine storage step custody absent"));
    assert!(matches!(h.engine.state()?.phase, Phase::Input(Some(_))));
    h.finish()
}

#[test]
fn composition_does_not_wait_for_a_busy_storage_owner() -> Result<()> {
    let mut h = Harness::new()?;
    let shared = Arc::clone(&h.engine.state);
    let held = shared.lock().map_err(|_| "Test state poisoned")?;
    assert!(h.step().is_err());
    assert!(h.engine.job.is_none());
    assert!(h.engine.parent.is_none());
    drop(held);
    h.finish()
}

#[test]
fn storage_panic_keeps_its_diagnostic_and_poisoning_does_not_prevent_cleanup() -> Result<()> {
    let mut h = Harness::new()?;
    let shared = Arc::clone(&h.engine.state);
    h.engine.job = Some(h.engine.client.submit_blocking(
        &h.engine.work,
        JobSpec {
            id: h.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(h.engine.until),
            reservation_bytes: 4096,
            replacement: None,
        },
        move |_| {
            let _held = shared.lock().unwrap_or_else(|poison| poison.into_inner());
            std::panic::resume_unwind(Box::new("engine storage fault"))
        },
    )?);
    let wake = h.wake.clone();
    observe(&wake, || match h.step() {
        Ok(()) => Ok(false),
        Err(cause) => {
            assert!(cause.to_string().contains("engine storage fault"));
            Ok(true)
        }
    })?;
    assert!(matches!(h.engine.state()?.phase, Phase::Input(Some(_))));
    h.finish()
}
