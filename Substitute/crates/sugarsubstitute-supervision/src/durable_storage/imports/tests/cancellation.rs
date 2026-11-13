// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual job cancellation independently of deterministic durable-phase expiration.
use super::*;
use crate::setup_storage::test_worker::pipeline::Pipeline;
use std::sync::Condvar;
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobClass, JobOutcome, JobSpec, Moment, Operation,
};
use sugarsubstitute_execution::qualification_time::ProgressGuard;

#[derive(Default)]
struct Checkpoint {
    ready: bool,
    release: bool,
}

#[test]
fn actual_cancellation_keeps_originals_and_admits_restart_only_under_a_fresh_owner() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let checkpoint = Arc::new((Mutex::new(Checkpoint::default()), Condvar::new()));
    let boundary = checkpoint.clone();
    let retained = Arc::new(Mutex::new(None::<tempfile::TempDir>));
    let original = retained.clone();
    let outcome = Arc::new(Mutex::new(None::<TestResult>));
    let result = outcome.clone();
    let mut job = pipeline.admit(|client, scope, id| {
        Ok(client.submit_blocking(
            scope,
            JobSpec {
                id,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(Moment::from_nanos(2_000_000_000)),
                reservation_bytes: 16 * 1024 * 1024,
                replacement: None,
            },
            move |context| {
                let assertion = (|| -> TestResult {
                    let mut fixture = Fixture::new(&context)?;
                    let (lock, wake) = &*boundary;
                    let mut state = crate::registry::lock(lock);
                    state.ready = true;
                    wake.notify_all();
                    let released =
                        ProgressGuard::process().wait_while(wake, state, |state| !state.release);
                    if let Some(stall) = released.stalled {
                        return Err(
                            format!("Cancellation boundary was never released: {stall}").into()
                        );
                    }
                    drop(released);
                    assert!(context.cancellation_requested());
                    let cause = fixture.copy(&context).err().ok_or("Cancelled import succeeded")?;
                    assert_eq!(cause.kind(), io::ErrorKind::Interrupted);
                    assert!(!fixture.target.exists());
                    assert!(!fixture.receipt()?.exists());
                    assert_eq!(
                        std::fs::read(fixture.source.path().join("document"))?,
                        b"original document"
                    );
                    *crate::registry::lock(&original) = fixture.temporary.take();
                    Ok(())
                })();
                *crate::registry::lock(&result) = Some(assertion);
                Ok::<_, io::Error>(())
            },
        )?)
    })?;
    let (lock, wake) = &*checkpoint;
    let prepared =
        ProgressGuard::process()
            .wait_while(wake, crate::registry::lock(lock), |state| !state.ready);
    let reached = prepared.stalled.is_none();
    drop(prepared);
    job.cancel();
    crate::registry::lock(lock).release = true;
    wake.notify_all();
    let terminal = pipeline.observe(|| job.try_take_completion())?;
    assert!(matches!(terminal.outcome, JobOutcome::Cancelled));
    drop(terminal);
    drop(job);
    if !reached {
        return Err("Import inputs were not prepared before cancellation".into());
    }
    crate::registry::lock(&outcome).take().ok_or("Actual cancellation assertions missing")??;
    drop(pipeline);
    let path = crate::registry::lock(&retained)
        .as_ref()
        .ok_or("Original data custody missing")?
        .path()
        .to_owned();
    test_worker::run(move |context, _| {
        let fixture = Fixture::reopen(&path)?;
        fixture.copy(context)?;
        assert_eq!(std::fs::read(fixture.target.join("document"))?, b"original document");
        assert_eq!(std::fs::read(fixture.source.path().join("document"))?, b"original document");
        fixture.copy(context)?;
        Ok(())
    })?;
    Ok(())
}
