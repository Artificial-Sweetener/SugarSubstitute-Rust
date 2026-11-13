// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Hold the actual original admitted main factory at an external boundary for resource tests.
use crate::{
    SymbolCleanupPermit,
    runtime_test_support::{Gate, Rig, TestResult, spec},
    scope::lock,
};
use std::{
    io,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::*;
pub(crate) struct LiveSymbol {
    pub(crate) rig: Rig,
    pub(crate) context: JobContext,
    pub(crate) _cleanup: SymbolCleanupPermit<(), io::Error>,
    #[cfg(windows)]
    pub(crate) cleanup_scope: Scope,
    /// Portable tests keep the cleanup scope open without reading it.
    #[cfg(not(windows))]
    _cleanup_scope: Scope,
    pub(crate) main: OwnedJob<(), io::Error>,
    release: Arc<Gate>,
}
impl LiveSymbol {
    pub(crate) fn new() -> TestResult<Self> {
        let rig = Rig::new()?;
        let cleanup_scope =
            rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        let cleanup_spec = spec(1, JobClass::InfrastructureLifecycle, 32_768)?;
        let mut main_spec = spec(2, JobClass::InfrastructureOrdinary, 8_159_232)?;
        main_spec.deadline = Some(Moment::from_nanos(10_000_000_000));
        let cleanup = rig
            .client
            .reserve_symbol_cleanup(&cleanup_scope, cleanup_spec, &rig.scope, main_spec, |_| {
                Ok::<_, io::Error>(())
            })
            .map_err(|rejected| rejected.error)?;
        let entered = Arc::new(Gate::default());
        let release = Arc::new(Gate::default());
        let stored = Arc::new(Mutex::new(None));
        let worker_entered = entered.clone();
        let worker_release = release.clone();
        let worker_stored = stored.clone();
        let main = rig
            .client
            .submit_symbol_helper(&rig.scope, main_spec, &cleanup, move |context| {
                *lock(&worker_stored) = Some(context);
                worker_entered.open();
                worker_release.wait().map_err(|cause| io::Error::other(cause.to_string()))?;
                Ok::<_, io::Error>(())
            })
            .map_err(|rejected| rejected.error)?;
        if let Err(cause) = entered.wait() {
            release.open();
            return Err(cause);
        }
        let context = lock(&stored).take().ok_or("Actual admitted main context absent")?;
        Ok(Self {
            rig,
            context,
            _cleanup: cleanup,
            #[cfg(windows)]
            cleanup_scope,
            #[cfg(not(windows))]
            _cleanup_scope: cleanup_scope,
            main,
            release,
        })
    }
    pub(crate) fn transport(&self) -> TestResult<crate::SymbolTransportReservation> {
        let partition = self
            .context
            .native_handles()
            .ok_or("Original ledger absent")?
            .take_partition(SymbolNativePartition::Transport)?;
        self.rig
            .client
            .symbol_transport_reservation(&self.context, partition)
            .map_err(|rejected| Box::new(rejected.error) as Box<dyn std::error::Error>)
    }
    pub(crate) fn complete(&mut self) -> TestResult<JobCompletion<(), io::Error>> {
        self.release.open();
        self.rig.completion(&mut self.main)
    }
}
impl Drop for LiveSymbol {
    fn drop(&mut self) {
        self.release.open();
    }
}
