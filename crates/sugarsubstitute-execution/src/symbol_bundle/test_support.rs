// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold genuine paired main and activated cleanup factories at external boundaries.
use crate::{
    SymbolCleanupPermit,
    runtime_test_support::{Gate, Rig, TestResult, spec},
    scope::lock,
};
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::*;
pub(crate) struct ForeignRelease;
impl MemoryRelease for ForeignRelease {
    fn release(&self, _: u64) {}
}
struct ForeignNative;
impl NativeHandleRelease for ForeignNative {
    fn release_symbol_reservation(&self) {}
    fn output_custody_changed(&self) {}
}
pub(crate) struct Ports(pub(crate) AtomicUsize);
impl CancelControl for Ports {
    fn is_requested(&self) -> bool {
        self.0.fetch_add(1, Ordering::AcqRel);
        true
    }
    fn cancelled(&self) -> WaitFuture {
        self.0.fetch_add(1, Ordering::AcqRel);
        Box::pin(std::future::pending())
    }
}
impl Clock for Ports {
    fn now(&self) -> Moment {
        self.0.fetch_add(1, Ordering::AcqRel);
        Moment::from_nanos(u64::MAX)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        self.0.fetch_add(1, Ordering::AcqRel);
        Box::pin(std::future::pending())
    }
}
pub(crate) fn supplied(
    actual: &JobContext,
    memory: MemoryLease,
    ledger: Option<NativeHandleLease>,
    ports: Arc<Ports>,
) -> JobContext {
    JobContext::new(
        actual.owner_id(),
        actual.scope_id(),
        actual.spec(),
        actual.deadline(),
        ports.clone(),
        ports,
        AdmittedJobResources::new(
            memory,
            ledger.map(|_| Box::new(ForeignNative) as Box<dyn NativeHandleRelease>),
        ),
    )
}
/// Release both external holds before the real ExecutionOwner fixture joins on every path.
struct Release(Arc<Gate>, Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.open();
        self.1.open();
    }
}
pub(crate) struct Pair {
    pub(crate) rig: Rig,
    _release: Release,
    pub(crate) context: JobContext,
    pub(crate) main: OwnedJob<(), io::Error>,
    pub(crate) permit: Option<SymbolCleanupPermit<(), io::Error>>,
    pub(crate) cleanup_scope: Scope,
    stored: Arc<Mutex<Option<JobContext>>>,
    cleanup_entered: Arc<Gate>,
    main_release: Arc<Gate>,
    cleanup_release: Arc<Gate>,
}
impl Pair {
    pub(crate) fn new() -> TestResult<Self> {
        let rig = Rig::new()?;
        let main_release = Arc::new(Gate::default());
        let cleanup_release = Arc::new(Gate::default());
        let release = Release(main_release.clone(), cleanup_release.clone());
        let cleanup_scope =
            rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        let cleanup_spec = spec(1, JobClass::InfrastructureLifecycle, 32768)?;
        let mut main_spec = spec(2, JobClass::InfrastructureOrdinary, 8159232)?;
        main_spec.deadline = Some(Moment::from_nanos(10000000000));
        let stored = Arc::new(Mutex::new(None));
        let cleanup_entered = Arc::new(Gate::default());
        let worker_stored = stored.clone();
        let worker_entered = cleanup_entered.clone();
        let worker_release = cleanup_release.clone();
        let permit = rig
            .client
            .reserve_symbol_cleanup(
                &cleanup_scope,
                cleanup_spec,
                &rig.scope,
                main_spec,
                move |context| {
                    *lock(&worker_stored) = Some(context);
                    worker_entered.open();
                    worker_release.wait().map_err(|cause| io::Error::other(cause.to_string()))?;
                    Ok::<_, io::Error>(())
                },
            )
            .map_err(|rejected| rejected.error)?;
        let main_context = Arc::new(Mutex::new(None));
        let main_entered = Arc::new(Gate::default());
        let worker_stored = main_context.clone();
        let worker_entered = main_entered.clone();
        let worker_release = main_release.clone();
        let main = rig
            .client
            .submit_symbol_helper(&rig.scope, main_spec, &permit, move |context| {
                *lock(&worker_stored) = Some(context);
                worker_entered.open();
                worker_release.wait().map_err(|cause| io::Error::other(cause.to_string()))?;
                Ok::<_, io::Error>(())
            })
            .map_err(|rejected| rejected.error)?;
        main_entered.wait()?;
        let context = lock(&main_context).take().ok_or("Actual original main absent")?;
        Ok(Self {
            rig,
            _release: release,
            context,
            main,
            permit: Some(permit),
            cleanup_scope,
            stored,
            cleanup_entered,
            main_release,
            cleanup_release,
        })
    }
    pub(crate) fn finish_main(&mut self) -> TestResult<JobCompletion<(), io::Error>> {
        self.main_release.open();
        self.rig.completion(&mut self.main)
    }
    pub(crate) fn activate(
        &mut self,
    ) -> TestResult<(OwnedJob<(), io::Error>, JobContext, crate::CleanupDeadline)> {
        let permit = self.permit.take().ok_or("Actual cleanup permit absent")?;
        let deadline = self
            .rig
            .client
            .freeze_symbol_cleanup_deadline(&permit, std::time::Duration::from_secs(15))?;
        let job = permit.begin(deadline).map_err(|rejected| rejected.error)?;
        self.cleanup_entered.wait()?;
        let context = lock(&self.stored).take().ok_or("Actual running cleanup absent")?;
        Ok((job, context, deadline))
    }
    pub(crate) fn finish_cleanup(
        &self,
        job: &mut OwnedJob<(), io::Error>,
    ) -> TestResult<JobCompletion<(), io::Error>> {
        self.cleanup_release.open();
        self.rig.completion(job)
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        // Custom Drop opens both gates before automatic field teardown reaches Rig.
        self.main_release.open();
        self.cleanup_release.open();
    }
}
