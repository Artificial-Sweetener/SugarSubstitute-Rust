// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise final receive admission while actual finite wait completions remain independently owned.
use super::*;
use crate::completion_wake::{Gate, Wake};
use std::{error::Error, io};
use sugarsubstitute_execution::ExecutionOwner;
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Frozen;
impl Clock for Frozen {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[test]
fn recovery_final_receive_prevents_new_native_read_while_actual_wait_completions_are_pending()
-> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(Frozen), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let original = Moment::from_nanos(15_000_000_000);
    let spec = |id| -> TestResult<JobSpec> {
        Ok(JobSpec {
            id: JobId::new(id)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(original),
            reservation_bytes: 4096,
            replacement: None,
        })
    };
    let gate = Arc::new(Gate::default());
    let _release = gate.guard();
    let boundary = gate.clone();
    let mut handler_wait = client.submit_blocking(&scope, spec(1)?, move |_| {
        boundary.hold();
        Ok::<(), io::Error>(())
    })?;
    gate.wait_entered();
    let mut desktop_wait = client.submit_blocking(&scope, spec(2)?, |_| Ok::<(), io::Error>(()))?;
    let mut received = RetirementReceipt::default();
    let mut reads = 0;
    received.admit_read(|| {
        reads += 1;
        Ok::<(), io::Error>(())
    })?;
    assert_eq!(reads, 1);
    // Passive phase input from the checked command owner, not an authenticated peer or native proof.
    received.receive(true)?;
    for _ in 0..2 {
        received.admit_read(|| {
            reads += 1;
            Err(io::Error::from_raw_os_error(109))
        })?;
        assert!(handler_wait.try_take_completion().is_none());
        assert!(!handler_wait.status().cancellation_requested);
    }
    assert_eq!(reads, 1);
    assert_eq!(scope.snapshot().reserved_bytes, 8192);
    gate.release();
    let handler = wake.completion(&client, &mut handler_wait)?;
    let desktop = wake.completion(&client, &mut desktop_wait)?;
    for done in [&handler, &desktop] {
        assert!(matches!(done.outcome, JobOutcome::Success(())));
        assert_eq!(done.spec.deadline, Some(original));
        assert_eq!(done.scope_id, scope.id());
    }
    drop((handler_wait, desktop_wait));
    drop(owner);
    // Completion guards, not the final protocol message, retain the original reservation through join.
    assert_eq!(scope.snapshot().reserved_bytes, 8192);
    drop((handler, desktop));
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn recovery_missing_or_rejected_final_receive_keeps_native_errors_and_admission() -> TestResult {
    let mut received = RetirementReceipt::default();
    assert_eq!(received.receive(false), Err(SessionCode::Order));
    assert!(!received.received());
    let mut reads = 0;
    let original = received
        .admit_read(|| {
            reads += 1;
            Err(io::Error::from_raw_os_error(109))
        })
        .err()
        .ok_or("Original unexpected native read failure lost")?;
    assert_eq!(reads, 1);
    assert_eq!(original.raw_os_error(), Some(109));
    assert!(received.read_open());
    received.receive(true)?;
    assert_eq!(received.receive(true), Err(SessionCode::Order));
    assert!(received.received());
    received.admit_read(|| {
        reads += 1;
        Err::<(), _>(original)
    })?;
    assert_eq!(reads, 1);
    Ok(())
}
