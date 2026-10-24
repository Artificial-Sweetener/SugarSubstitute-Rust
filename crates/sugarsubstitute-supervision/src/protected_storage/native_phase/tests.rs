// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise optional installation deadlines through actual execution admission and native storage.
use super::*;
use crate::setup_storage::{SelectionStore, test_worker};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::{
    execution::{ExecutionPort, JobClass, JobOutcome, JobSpec, Operation, ScopeId, ScopeSpec},
    installation::{InstallationId, InstallationRecord},
};

type Failure = Box<dyn std::error::Error + Send + Sync>;

#[test]
fn protected_installation_without_deadline_publishes_and_reopens_selection() -> Result<(), Failure>
{
    let mut pipeline = test_worker::pipeline::Pipeline::new()?;
    let mut job = pipeline.admit(|client, scope, id| {
        Ok(client.submit_blocking(
            scope,
            JobSpec {
                id,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureOrdinary,
                deadline: None,
                reservation_bytes: 16 * 1024 * 1024,
                replacement: None,
            },
            |context| -> Result<(), Failure> {
                assert!(context.deadline().is_none());
                let root = tempfile::tempdir()?;
                std::fs::write(
                    root.path().join(".substitute-installation.json"),
                    br#"{"schema_version":1,"product":"SugarSubstitute"}"#,
                )?;
                let initial = InstallationRecord::empty(InstallationId::new("1".repeat(32))?);
                let store = SelectionStore::initialize(root.path(), &initial, &context)?;
                assert_eq!(store.load(&context)?, Some(initial.clone()));
                drop(store);
                assert_eq!(
                    SelectionStore::open(root.path(), &context)?.load(&context)?,
                    Some(initial)
                );
                Ok(())
            },
        )?)
    })?;
    match pipeline.observe(|| job.try_take_completion())?.outcome {
        JobOutcome::Success(()) => Ok(()),
        JobOutcome::Failure(cause) => Err(cause),
        other => Err(format!("Installation job ended without its result: {other:?}").into()),
    }
}

#[test]
fn protected_storage_still_rejects_an_expired_explicit_deadline() -> Result<(), Failure> {
    test_worker::run_reserved_expiring(2 * 1024 * 1024, |context, clock| {
        let budget = Budget::Protected(Arc::new(context.memory().claim(1024 * 1024)?));
        let mut phase = NativePhase::ordinary(&budget)?;
        phase.context(context)?;
        clock.expire();
        assert!(phase.context(context).is_err());
        Ok(())
    })
}

#[test]
fn protected_storage_without_deadline_still_observes_scope_cancellation() -> Result<(), Failure> {
    let mut pipeline = test_worker::pipeline::Pipeline::new()?;
    let rejected = Arc::new(AtomicBool::new(false));
    let observed = rejected.clone();
    let mut job = pipeline.admit(|client, scope, id| {
        let scope = Arc::new(
            client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(scope))?,
        );
        let scope_to_cancel = scope.clone();
        Ok(client.submit_blocking(
            &scope,
            JobSpec {
                id,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureOrdinary,
                deadline: None,
                reservation_bytes: 2 * 1024 * 1024,
                replacement: None,
            },
            move |context| -> Result<(), Failure> {
                let budget = Budget::Protected(Arc::new(context.memory().claim(1024 * 1024)?));
                let mut phase = NativePhase::ordinary(&budget)?;
                phase.context(&context)?;
                scope_to_cancel.cancel();
                observed.store(phase.context(&context).is_err(), Ordering::SeqCst);
                Ok(())
            },
        )?)
    })?;
    assert!(matches!(
        pipeline.observe(|| job.try_take_completion())?.outcome,
        JobOutcome::Cancelled
    ));
    assert!(rejected.load(Ordering::SeqCst));
    Ok(())
}
