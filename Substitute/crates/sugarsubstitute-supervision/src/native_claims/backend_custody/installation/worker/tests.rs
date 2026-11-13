// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real signed storage and execution jobs; wake delivery is the external native-loop boundary.
use super::*;
use crate::{
    native_claims::files::Budget,
    setup_storage::{self, test_worker},
};
use std::{
    num::NonZeroUsize,
    sync::{Condvar, Mutex},
};
use sugarsubstitute_contracts::installation::HostExit;
use sugarsubstitute_execution::ExecutionOwner;
type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *crate::registry::lock(&self.0) += 1;
        self.1.notify_all();
        Ok(())
    }
}
struct Rig {
    service: BackendCustodyInstallation,
    control: Control,
    client: ExecutionClient,
    scope: Scope,
    clock: Arc<test_worker::TestClock>,
    wake: Arc<Wake>,
    _execution: ExecutionOwner,
    /// The creator's live handoff, which the witness adopts from; kept for the rig's lifetime.
    _sender: setup_storage::TrialTransfer,
    _root: tempfile::TempDir,
}
impl Rig {
    fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let wake = Arc::new(Wake::default());
        let retained = Arc::new(Mutex::new(None));
        let output = retained.clone();
        let notified = wake.clone();
        test_worker::run_reserved(
            setup_storage::SIGNED_INSTALLER_RESERVATION_BYTES,
            move |context, _| {
                let root = tempfile::tempdir()?;
                let transfer = setup_storage::test_package::trial_transfer(root.path(), context)?;
                let input = [
                    transfer.handles()[0].try_clone_to_owned()?,
                    transfer.handles()[1].try_clone_to_owned()?,
                ];
                let memory = Budget::Protected(Arc::new(context.memory().claim(2 * 1024 * 1024)?));
                let control = Control::new(memory, transfer.directory().to_owned(), notified);
                *output.lock().map_err(|_| "Fixture retention poisoned")? =
                    Some((root, control, input, transfer));
                Ok(())
            },
        )?;
        let (root, control, input, sender) = retained
            .lock()
            .map_err(|_| "Fixture retention poisoned")?
            .take()
            .ok_or("Fixture absent")?;
        let clock = Arc::new(test_worker::TestClock::default());
        let execution = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = execution.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let service = control.worker()?;
        control.admit(input, Moment::from_nanos(1_000_000_000), wake.clone())?;
        Ok(Self {
            service,
            control,
            client,
            scope,
            clock,
            wake,
            _execution: execution,
            _sender: sender,
            _root: root,
        })
    }

    fn advance(&mut self) -> Result<(), ClaimError> {
        self.service.advance(
            &self.client,
            &self.scope,
            JobId::new(4).map_err(|cause| ClaimError::new("fixture_id", cause))?,
            JobId::new(5).map_err(|cause| ClaimError::new("fixture_id", cause))?,
        )
    }

    fn completion(&self) -> Result<CompletionBatch, Box<dyn std::error::Error + Send + Sync>> {
        loop {
            let observed = *crate::registry::lock(&self.wake.0);
            let batch = self.client.drain_completions(NonZeroUsize::MAX)?;
            if !batch.notifications.is_empty() {
                return Ok(batch);
            }
            let guard = crate::registry::lock(&self.wake.0);
            drop(
                self.wake
                    .1
                    .wait_while(guard, |current| *current == observed)
                    .map_err(|_| "Wake poisoned")?,
            );
        }
    }
}

#[test]
fn readiness_and_settlement_require_consumed_completions_and_original_cutoff() -> TestResult {
    let mut rig = Rig::new()?;
    assert!(!rig.control.ready()?);
    assert!(rig.control.worker().is_err());
    rig.advance()?;
    let batch = rig.completion()?;
    assert!(!rig.control.ready()?);
    assert!(crate::registry::lock(&rig.control.0.data).witness.is_some());
    rig.service.observe(&batch)?;
    assert!(rig.control.ready()?);
    assert!(!rig.control.settle(Moment::from_nanos(3_000_000_000), None)?);
    assert!(rig.control.ready()?);
    let cutoff = Moment::from_nanos(4_000_000_000);
    assert!(!rig.control.settle(cutoff, Some(HostExit::Finished))?);
    assert!(!rig.control.settle(Moment::from_nanos(9_000_000_000), Some(HostExit::Finished))?);
    assert_eq!(crate::registry::lock(&rig.control.0.data).until, Some(cutoff));
    rig.advance()?;
    let batch = rig.completion()?;
    assert!(!rig.control.settle(cutoff, Some(HostExit::Finished))?);
    rig.service.observe(&batch)?;
    assert!(rig.control.settle(cutoff, Some(HostExit::Finished))?);
    assert!(crate::registry::lock(&rig.control.0.data).witness.is_some());
    Ok(())
}

#[test]
fn discarded_admission_result_keeps_witness_without_acknowledging_readiness() -> TestResult {
    let mut rig = Rig::new()?;
    let retained = rig.control.clone();
    rig.clock.observe_checks(move || crate::registry::lock(&retained.0.data).witness.is_some())?;
    rig.advance()?;
    let batch = rig.completion()?;
    assert_eq!(batch.notifications[0].kind, TerminalKind::TimedOut);
    assert!(crate::registry::lock(&rig.control.0.data).witness.is_some());
    assert!(rig.service.observe(&batch).is_err());
    assert!(rig.control.ready().is_err());
    assert!(rig.advance().is_err());
    Ok(())
}

#[test]
fn changed_completion_notice_cannot_acknowledge_actual_storage_success() -> TestResult {
    let mut rig = Rig::new()?;
    rig.advance()?;
    let mut batch = rig.completion()?;
    batch.notifications[0].kind = TerminalKind::Cancelled;
    assert!(rig.service.observe(&batch).is_err());
    assert!(rig.control.ready().is_err());
    assert!(crate::registry::lock(&rig.control.0.data).witness.is_some());
    Ok(())
}

#[test]
fn closing_during_admission_keeps_first_cutoff_and_survives_native_scope_cancellation() -> TestResult
{
    let mut rig = Rig::new()?;
    let native =
        rig.client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let cutoff = Moment::from_nanos(4_000_000_000);
    assert!(!rig.control.settle(cutoff, Some(HostExit::Finished))?);
    native.cancel();
    rig.advance()?;
    let batch = rig.completion()?;
    rig.service.observe(&batch)?;
    assert!(rig.control.ready()?);
    assert!(!rig.control.settle(Moment::from_nanos(9_000_000_000), Some(HostExit::Finished))?);
    assert_eq!(crate::registry::lock(&rig.control.0.data).until, Some(cutoff));
    rig.advance()?;
    let batch = rig.completion()?;
    rig.service.observe(&batch)?;
    assert!(rig.control.settle(cutoff, Some(HostExit::Finished))?);
    assert!(native.snapshot().cancellation_requested);
    Ok(())
}
