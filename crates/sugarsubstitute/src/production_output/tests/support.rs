// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Use the real central executor, clock, scopes, notices and worker joins.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
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
pub(super) struct Rig {
    pub(super) owner: sugarsubstitute_execution::ExecutionOwner,
    pub(super) client: ExecutionClient,
    pub(super) scope: Scope,
    wake: Arc<Wake>,
}
impl Rig {
    pub(super) fn new() -> TestResult<Self> {
        let wake = Arc::new(Wake::default());
        let owner = sugarsubstitute_execution::ExecutionOwner::new(Arc::new(Clock), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { owner, client, scope, wake })
    }
    pub(super) fn result<T: Send + 'static, E: Error + Send + Sync + 'static>(
        &self,
        job: &mut OwnedJob<T, E>,
    ) -> TestResult<T> {
        let mut noticed = false;
        loop {
            let mut count = self.wake.count.lock().map_err(|_| "Wake poisoned")?;
            let before = *count;
            let batch =
                self.client.drain_completions(NonZeroUsize::new(64).ok_or("Zero drain")?)?;
            for n in batch.notifications {
                if (n.scope_id, n.job_id) == (job.scope_id(), job.spec().id) {
                    noticed = true
                }
            }
            if noticed && let Some(done) = job.try_take_completion() {
                return match done.outcome {
                    JobOutcome::Success(v) => Ok(v),
                    JobOutcome::Failure(e) => Err(Box::new(e)),
                    other => Err(format!("Actual terminal {:?}", other.kind()).into()),
                };
            }
            if *count == before {
                let next = self
                    .wake
                    .changed
                    .wait_timeout(count, Duration::from_secs(10))
                    .map_err(|_| "Wake poisoned")?;
                count = next.0;
                if next.1.timed_out() {
                    return Err("Actual completion did not arrive".into());
                }
            }
            drop(count);
        }
    }
}
pub(super) fn binding() -> TestResult<OutputBinding> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([1; 16])?,
    };
    Ok(OutputBinding {
        resource: ResourceKey {
            owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
        run: RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? },
        connection: ConnectionId::new(1)?,
        lease: LeaseReference {
            resource: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
            lease: LeaseId::new(1)?,
        },
    })
}
pub(super) fn header(binding: OutputBinding, id: u64) -> TestResult<WireHeader> {
    Ok(WireHeader {
        run: binding.run,
        connection: Some(binding.connection),
        request: Some(RequestId::new(id)?),
        deadline_qpc: 100,
    })
}
pub(super) fn output(rig: &Rig) -> TestResult<OutputOwner> {
    let mut job = OutputOwner::submit_new(
        &rig.client,
        &rig.scope,
        JobId::new(1)?,
        binding()?,
        Moment::from_nanos(100),
    )?;
    rig.result(&mut job)
}
