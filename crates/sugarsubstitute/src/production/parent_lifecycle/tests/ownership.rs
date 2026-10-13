// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct actor storage through an actual admitted job; matching facts grant no IPC authority.
use super::{ParentLifecycle, Result, identity};
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::{
    configuration::ProfileId,
    execution::{
        Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobId, JobOutcome, JobSpec,
        Moment, Operation, ScopeId, ScopeSpec, WaitFuture,
    },
    lifecycle::{ConnectionId, LeaseId, LeaseKey, ResourceGeneration, ResourceId, ResourceKey},
};
use sugarsubstitute_execution::{ExecutionOwner, windows_lifecycle_pipe::LifecyclePipeDeadline};

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(1)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    ready: Mutex<bool>,
    changed: Condvar,
}
impl Wake {
    fn wait(&self) -> Result<()> {
        let ready = self.ready.lock().map_err(|cause| std::io::Error::other(cause.to_string()))?;
        let (mut ready, timeout) = self
            .changed
            .wait_timeout_while(ready, Duration::from_secs(10), |ready| !*ready)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && !*ready {
            return Err("Actual completion wake did not arrive".into());
        }
        *ready = false;
        Ok(())
    }
}
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        let mut ready = self.ready.lock().map_err(|cause| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(std::io::Error::other(cause.to_string())),
            )
        })?;
        *ready = true;
        self.changed.notify_one();
        Ok(())
    }
}

#[test]
fn original_actor_charge_outlives_job_and_frontend_wait_keeps_original_cutoff() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(1000)),
            reservation_bytes: ParentLifecycle::storage_bytes(),
            replacement: None,
        },
        |context| context.memory().claim(ParentLifecycle::storage_bytes()),
    )?;
    let completion = loop {
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        wake.wait()?;
        let batch =
            client.drain_completions(NonZeroUsize::new(32).ok_or("Invalid batch bound")?)?;
        for notice in batch.notifications {
            assert_eq!(notice.job_id, job.spec().id);
            assert_eq!(notice.scope_id, scope.id());
        }
    };
    let JobOutcome::Success(claim) = completion.outcome else {
        return Err("Actual actor storage admission did not succeed".into());
    };
    let id = identity()?;
    let connection = ConnectionId::new(1)?;
    let mut actor = ParentLifecycle::new(
        id.run,
        id.launch,
        ProfileId::new("Actor_1".into())?,
        connection,
        LifecyclePipeDeadline::new(1000, 1_000_000_000)?,
        claim,
    )?;
    drop(completion.memory);
    drop(job);
    // Join real workers before asserting final claim custody; terminal delivery alone is earlier.
    drop(executor);
    assert_eq!(scope.snapshot().reserved_bytes, ParentLifecycle::storage_bytes());
    assert!(!actor.native_retired());
    assert!(!actor.settled());
    assert!(actor.retired_registration(id.run, connection)?.is_none());
    let foreign = sugarsubstitute_contracts::lifecycle::RunKey {
        generation: sugarsubstitute_contracts::lifecycle::RunGeneration::new(2)?,
        ..id.run
    };
    assert!(matches!(
        actor.retired_registration(foreign, connection),
        Err(super::ParentError::ForeignPeer)
    ));
    assert!(matches!(
        actor.retired_registration(id.run, ConnectionId::new(2)?),
        Err(super::ParentError::ForeignPeer)
    ));
    assert_eq!(actor.main_deadline(), None);
    let lease = LeaseKey {
        run: id.run,
        connection,
        id: LeaseId::new(1)?,
        resource: ResourceKey {
            owner: id.run.owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
    };
    actor.main.begin(id, lease, Moment::from_nanos(77))?;
    assert_eq!(actor.main_deadline(), Some(Moment::from_nanos(77)));
    assert_eq!(actor.next_deadline(), Some(Moment::from_nanos(77)));
    actor.main.retire(id)?;
    assert_eq!(actor.main_deadline(), None);
    assert!(!actor.native_retired());
    drop(actor);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
