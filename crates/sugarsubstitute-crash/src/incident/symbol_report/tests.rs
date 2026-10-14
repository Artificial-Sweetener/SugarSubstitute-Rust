// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise genuine held-job admission, source custody and exact completion routing without native selection.
use super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
type TestResult = Result<(), Box<dyn Error + Send + Sync>>;
struct Clock;
impl sugarsubstitute_contracts::execution::Clock for Clock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
struct Rig {
    owner: ExecutionOwner,
    client: ExecutionClient,
    main: Scope,
    cleanup: Scope,
    wake: Arc<Wake>,
}
impl Rig {
    fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(Clock), wake.clone())?;
        let client = owner.client();
        let main = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let cleanup =
            client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        Ok(Self { owner, client, main, cleanup, wake })
    }
    fn reserved(&self) -> Result<SymbolReportService, Box<dyn Error + Send + Sync>> {
        Ok(SymbolReportService::reserve(
            self.client.clone(),
            &self.cleanup,
            JobId::new(1)?,
            &self.main,
            JobId::new(2)?,
            Moment::from_nanos(60_000_000_017),
        )?)
    }
    fn settle(&self, service: &mut SymbolReportService) -> TestResult {
        loop {
            let mut count = self.wake.count.lock().map_err(|_| "Wake poisoned")?;
            let before = *count;
            let batch =
                self.client.drain_completions(NonZeroUsize::new(32).ok_or("Zero drain")?)?;
            service.capture(&batch);
            if service.cleanup_terminal {
                return Ok(());
            }
            if *count == before {
                let next = self
                    .wake
                    .changed
                    .wait_timeout(count, Duration::from_secs(10))
                    .map_err(|_| "Wake poisoned")?;
                count = next.0;
                if next.1.timed_out() {
                    return Err("Actual held factory did not retire".into());
                }
            }
            drop(count);
        }
    }
}
#[test]
fn unsubmitted_cancellation_observes_actual_factory_retirement_and_last_source_charge() -> TestResult
{
    let rig = Rig::new()?;
    let mut service = rig.reserved()?;
    let retained = service.source.clone();
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 32768);
    assert!(service.take_result().is_none());
    service.cancel_unsubmitted()?;
    rig.settle(&mut service)?;
    let result = service.take_result().ok_or("Missing outcome")?;
    assert!(matches!(result, SymbolReportResult::Failed(_)));
    assert!(service.take_result().is_none());
    assert_eq!(service.source.with(|state| state.data.is_none()), Some(true));
    drop(service);
    drop(result);
    drop(rig.owner);
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 32768);
    drop(retained);
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn foreign_completion_does_not_consume_current_source_or_held_job() -> TestResult {
    let rig = Rig::new()?;
    let mut service = rig.reserved()?;
    let foreign = CompletionBatch {
        notifications: vec![CompletionNotification {
            scope_id: rig.main.id(),
            job_id: JobId::new(2)?,
            kind: TerminalKind::Success,
            replacement: None,
        }],
        remaining: 0,
    };
    assert!(!service.capture(&foreign));
    assert!(!service.main_terminal);
    assert!(!service.cleanup_terminal);
    assert_eq!(rig.cleanup.snapshot().queued, 1);
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 32768);
    service.cancel_unsubmitted()?;
    rig.settle(&mut service)?;
    drop(service);
    drop(rig.owner);
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn excessive_original_budget_rejects_before_source_admission() -> TestResult {
    let rig = Rig::new()?;
    let rejected = SymbolReportService::reserve(
        rig.client.clone(),
        &rig.cleanup,
        JobId::new(1)?,
        &rig.main,
        JobId::new(2)?,
        Moment::from_nanos(60_000_000_018),
    );
    assert!(matches!(rejected, Err(AdmissionError::InvalidSymbolAdmission)));
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 0);
    assert_eq!(rig.cleanup.snapshot().queued, 0);
    Ok(())
}

#[test]
fn unbound_request_rejects_before_actual_main_or_native_admission() -> TestResult {
    let rig = Rig::new()?;
    let mut service = rig.reserved()?;
    assert!(matches!(service.submit(&rig.main), Err(AdmissionError::InvalidSymbolAdmission)));
    assert_eq!(rig.main.snapshot().reserved_bytes, 0);
    assert_eq!(service.source.with(|state| state.started), Some(false));
    service.cancel_unsubmitted()?;
    rig.settle(&mut service)?;
    drop(service);
    drop(rig.owner);
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn actual_provider_buffer_owner_closes_under_same_pair_and_retains_charge_through_result()
-> TestResult {
    let rig = Rig::new()?;
    let mut service = rig.reserved()?;
    let source = service.source.clone();
    let client = rig.client.clone();
    let held = service.held.as_ref().ok_or("Missing actual held cleanup")?;
    let job = rig
        .client
        .submit_symbol_helper(&rig.main, service.main_spec, held, move |context| {
            let admission = client.symbol_provider_admission(&context).map_err(|_| ())?;
            let provider =
                crate::incident::DbgHelpSymbolProvider::new(admission).map_err(|_| ())?;
            let original = provider.original_origin().cloned().ok_or(())?;
            let data = super::super::source::DataOwner::new(provider, original);
            source.update(|state| {
                state.started = true;
                state.before_effects = false;
                state.data = Some(data);
            });
            Ok::<_, ()>(())
        })
        .map_err(|rejection| rejection.error)?;
    service.main = Some(job);
    service.submitted = true;
    loop {
        let count = rig.wake.count.lock().map_err(|_| "Wake poisoned")?;
        let before = *count;
        let batch = rig.client.drain_completions(NonZeroUsize::new(32).ok_or("Zero drain")?)?;
        service.capture(&batch);
        if service.main_terminal {
            drop(count);
            break;
        }
        let next = rig
            .wake
            .changed
            .wait_timeout_while(count, Duration::from_secs(10), |count| *count == before)
            .map_err(|_| "Wake poisoned")?;
        if next.1.timed_out() {
            return Err("Actual provider Main did not terminate".into());
        }
    }
    assert_eq!(rig.main.snapshot().reserved_bytes, 8_159_232);
    assert!(service.advance_cleanup()?);
    rig.settle(&mut service)?;
    let result = service.take_result().ok_or("Missing actual cleanup result")?;
    match &result {
        SymbolReportResult::Failed(failure) => assert!(failure.settled()),
        SymbolReportResult::Verified(_) => return Err("No actual dump was read".into()),
    }
    drop(service);
    drop(rig.owner);
    assert_eq!(rig.main.snapshot().reserved_bytes, 8_159_232);
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 32768);
    drop(result);
    assert_eq!(rig.main.snapshot().reserved_bytes, 0);
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
