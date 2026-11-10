// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove the actual admitted memory claim survives the final real Work alias.
use super::*;
use crate::completion_wake::Wake;
use std::error::Error;
use sugarsubstitute_execution::ExecutionOwner;
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
fn root_recovery_actual_work_retains_admitted_funding_until_last_worker_alias_is_dropped()
-> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(Arc::new(Frozen), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let bytes = RootRecoverySession::required_storage_bytes();
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: bytes,
            replacement: None,
        },
        move |context| context.memory().claim(bytes),
    )?;
    let done = wake.completion(&client, &mut job)?;
    let lease = done.memory.clone();
    let claim = match done.outcome {
        JobOutcome::Success(claim) => claim,
        _ => return Err("Actual funding job failed".into()),
    };
    let funding = Arc::new(Funding { _memory: claim });
    let work = Arc::new(Mutex::new(Work {
        desktop_watch: None,
        desktop: None,
        desktop_prepared: false,
        enrollment: None,
        handler_permit: None,
        handler: None,
        cancelled_ui: None,
        cancelled_handler: None,
        ui_terminal: None,
        handler_terminal: None,
        failure: [None, None],
        _funding: funding.clone(),
    }));
    let worker = work.clone();
    drop(funding);
    drop(work);
    assert!(lease.claim(bytes).is_err());
    drop(worker);
    let restored = lease.claim(bytes)?;
    drop(restored);
    drop(lease);
    drop(job);
    drop(scope);
    // This finite owner test creates no process/pipe registration and claims no native retirement.
    wake.finish_shutdown(&mut owner, &client, Moment::from_nanos(15_000_000_000))?;
    Ok(())
}
