// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Use the real central executor, clock, scopes, notices and worker joins.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionClient;
pub(super) struct Clock(std::sync::atomic::AtomicU64);
impl sugarsubstitute_contracts::execution::Clock for Clock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(std::sync::atomic::Ordering::Acquire))
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
    pub(super) clock: Arc<Clock>,
}
impl Rig {
    pub(super) fn new() -> TestResult<Self> {
        let wake = Arc::new(Wake::default());
        let clock = Arc::new(Clock(std::sync::atomic::AtomicU64::new(17)));
        let owner = sugarsubstitute_execution::ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { owner, client, scope, wake, clock })
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
pub(super) fn identity() -> TestResult<MainIdentity> {
    use sugarsubstitute_contracts::lifecycle::*;
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([1; 16])?,
    };
    Ok(MainIdentity {
        run: RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? },
        launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
pub(super) fn pending(identity: MainIdentity, id: u64) -> TestResult<Pending> {
    use sugarsubstitute_contracts::lifecycle::{ConnectionId, RequestId};
    Ok(Pending {
        header: WireHeader {
            run: identity.run,
            connection: Some(ConnectionId::new(1)?),
            request: Some(RequestId::new(id)?),
            deadline_qpc: 100,
        },
        admitted: true,
    })
}
pub(super) fn transaction(rig: &Rig) -> TestResult<ProfileTransaction> {
    use sugarsubstitute_contracts::configuration::{LoopbackOrigin, ProfileId};
    use sugarsubstitute_desktop::{MainCaptionLabels, MainWindowSettings, ServedFrontendVersions};
    let id = identity()?;
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: MainWindowRequest::RESERVATION_BYTES,
            replacement: None,
        },
        move |context| -> Result<ProfileTransaction, ProfileError> {
            let request = MainWindowRequest::new(
                id,
                LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
                ProfileId::new("primary".to_owned())?,
                std::path::PathBuf::from(r"E:\profile"),
                ServedFrontendVersions::new("1.0.0".to_owned(), "1.0.0".to_owned())?,
                MainWindowSettings {
                    recovery: sugarsubstitute_desktop::MainRecoveryLabels {
                        retry: sugarsubstitute_contracts::localization::messages("en").retry,
                        retry_ready: sugarsubstitute_contracts::localization::messages("en")
                            .retry_ready,
                    },
                    logical_width: 800,
                    logical_height: 600,
                    dark: true,
                    labels: MainCaptionLabels {
                        minimize: "Minimize",
                        maximize: "Maximize",
                        restore: "Restore",
                        close: "Close",
                    },
                },
                context.memory().clone(),
            )?;
            ProfileTransaction::new(Box::new(request), Moment::from_nanos(100), &context)
        },
    )?;
    rig.result(&mut job)
}

impl Rig {
    pub(super) fn expire(&self) {
        self.clock.0.store(100, std::sync::atomic::Ordering::Release);
    }
}
