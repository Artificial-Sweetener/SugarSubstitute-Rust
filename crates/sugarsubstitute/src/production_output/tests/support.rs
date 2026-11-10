// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Use the real central executor, clock, scopes, notices and worker joins.
use super::*;
use crate::completion_wake::Wake;
use std::{num::NonZeroUsize, sync::Arc};
struct Clock;
impl sugarsubstitute_contracts::execution::Clock for Clock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
pub(super) struct Rig {
    pub(super) owner: sugarsubstitute_execution::ExecutionOwner,
    pub(super) client: ExecutionClient,
    pub(super) scope: Scope,
    wake: Arc<Wake>,
}
impl Rig {
    /// Check settlement and all real worker joins after the owned terminal is disposed.
    pub(super) fn finish(&mut self) -> TestResult {
        self.owner.request_shutdown(Moment::from_nanos(100));
        loop {
            let before = self.wake.revision();
            self.client.drain_completions(NonZeroUsize::MIN.saturating_add(63))?;
            if self.owner.poll_shutdown()?.settled {
                return Ok(self.owner.finish_shutdown()?);
            }
            self.wake.wait_after(before);
        }
    }
    /// Observe the actual owner's completion notice without taking its terminal custody.
    pub(super) fn notice(&self, output: &mut OutputOwner) -> TestResult {
        self.wake.until(|| {
            let batch = match self.client.drain_completions(NonZeroUsize::MIN.saturating_add(63)) {
                Ok(batch) => batch,
                Err(cause) => return Some(Err::<(), Box<dyn Error + Send + Sync>>(cause.into())),
            };
            output.capture(&batch);
            output.noticed.is_some().then_some(Ok(()))
        })
    }
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
        let done = self.wake.until(|| {
            let batch = match self.client.drain_completions(NonZeroUsize::MIN.saturating_add(63)) {
                Ok(batch) => batch,
                Err(cause) => return Some(Err::<_, Box<dyn Error + Send + Sync>>(cause.into())),
            };
            for n in batch.notifications {
                if (n.scope_id, n.job_id) == (job.scope_id(), job.spec().id) {
                    noticed = true
                }
            }
            if noticed { job.try_take_completion().map(Ok) } else { None }
        })?;
        match done.outcome {
            JobOutcome::Success(v) => Ok(v),
            JobOutcome::Failure(e) => Err(Box::new(e)),
            other => Err(format!("Actual terminal {:?}", other.kind()).into()),
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
