// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove the actual admitted memory claim survives the final real Work alias.
use super::*;
use std::{
    error::Error,
    sync::Condvar,
    time::{Duration, Instant},
};
use sugarsubstitute_execution::ExecutionOwner;
#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(std::io::Error::other("Recovery funding observer poisoned")),
            )
        })?;
        *revision = revision.saturating_add(1);
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    fn revision(&self) -> Result<u64, Box<dyn Error>> {
        Ok(*self.revision.lock().map_err(|_| "Observer poisoned")?)
    }
    fn after(&self, revision: u64, until: Instant) -> Result<bool, Box<dyn Error>> {
        let mut current = self.revision.lock().map_err(|_| "Observer poisoned")?;
        while *current <= revision {
            let Some(left) = until.checked_duration_since(Instant::now()) else {
                return Ok(false);
            };
            let (next, timed) =
                self.changed.wait_timeout(current, left).map_err(|_| "Observer poisoned")?;
            current = next;
            if timed.timed_out() && *current <= revision {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
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
    let until =
        Instant::now().checked_add(Duration::from_secs(10)).ok_or("Failure deadline overflow")?;
    let done = loop {
        let revision = wake.revision()?;
        if let Some(done) = job.try_take_completion() {
            break done;
        }
        if !wake.after(revision, until)? {
            break job.try_take_completion().ok_or("Actual funding job missing")?;
        }
    };
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
    owner.request_shutdown(Moment::from_nanos(15_000_000_000));
    loop {
        let revision = wake.revision()?;
        let _actual =
            client.drain_completions(NonZeroUsize::new(32).unwrap_or(NonZeroUsize::MIN))?;
        if owner.poll_shutdown()?.settled {
            owner.finish_shutdown()?;
            break;
        }
        if !wake.after(revision, until)? {
            if owner.poll_shutdown()?.settled {
                owner.finish_shutdown()?;
                break;
            }
            return Err("Actual execution worker settlement missing".into());
        }
    }
    Ok(())
}
