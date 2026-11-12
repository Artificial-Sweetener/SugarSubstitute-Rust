// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose independent genuine cleanup controls and guarded external worker holds.
use crate::{
    ReportExportCleanupPermit, SymbolCleanupPermit,
    runtime_test_support::{Gate, Rig, TestResult, spec},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::*;
pub(crate) struct Setup {
    pub(crate) rig: Rig,
    _source_cleanup_scope: Scope,
    _receiving_cleanup_scope: Scope,
    pub(crate) receiving_scope: Scope,
    pub(crate) source_spec: JobSpec,
    pub(crate) receiving_spec: JobSpec,
    pub(crate) source_cleanup: SymbolCleanupPermit<bool, u8>,
    pub(crate) receiving_cleanup: ReportExportCleanupPermit<bool, u8>,
    pub(crate) release: Option<Arc<Gate>>,
}
impl Drop for Setup {
    fn drop(&mut self) {
        if let Some(release) = &self.release {
            release.open();
        }
    }
}
impl Setup {
    pub(crate) fn new() -> TestResult<Self> {
        Self::after(Rig::new()?, 1)
    }
    pub(crate) fn after(rig: Rig, first: u64) -> TestResult<Self> {
        Self::after_main(rig, first, first + 2)
    }
    pub(crate) fn after_main(rig: Rig, first: u64, main_first: u64) -> TestResult<Self> {
        let source_cleanup_scope =
            rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        let receiving_scope =
            rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
        let receiving_cleanup_scope =
            rig.client.create_scope(ScopeSpec { id: ScopeId::new(4)?, deadline: None }, None)?;
        let mut source_spec = spec(main_first, JobClass::InfrastructureOrdinary, 8_159_232)?;
        source_spec.deadline = Some(Moment::from_nanos(10_000_000_000));
        let mut receiving_spec = spec(main_first + 1, JobClass::InfrastructureOrdinary, 131_072)?;
        receiving_spec.deadline = source_spec.deadline;
        let source_cleanup = rig
            .client
            .reserve_symbol_cleanup(
                &source_cleanup_scope,
                spec(first, JobClass::InfrastructureLifecycle, 32_768)?,
                &rig.scope,
                source_spec,
                |_| Ok::<_, u8>(true),
            )
            .map_err(|r| r.error)?;
        let receiving_cleanup = rig
            .client
            .reserve_report_export_cleanup(
                &receiving_cleanup_scope,
                spec(first + 1, JobClass::InfrastructureLifecycle, 32_768)?,
                &receiving_scope,
                receiving_spec,
                |_| Ok::<_, u8>(true),
            )
            .map_err(|r| r.error)?;
        Ok(Self {
            _source_cleanup_scope: source_cleanup_scope,
            _receiving_cleanup_scope: receiving_cleanup_scope,
            rig,
            receiving_scope,
            source_spec,
            receiving_spec,
            source_cleanup,
            receiving_cleanup,
            release: None,
        })
    }
    pub(crate) fn submit<M>(&self, factory: M) -> super::ReportExportPairSubmission<u8, M>
    where
        M: FnOnce(JobContext, JobContext) -> Result<bool, u8> + Send + 'static,
    {
        self.rig.client.submit_report_export_pair(
            crate::ReportExportPairRequest::new(
                &self.rig.scope,
                self.source_spec,
                &self.source_cleanup,
                &self.receiving_scope,
                self.receiving_spec,
                &self.receiving_cleanup,
            ),
            factory,
        )
    }
}
pub(crate) struct Release(pub(crate) Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.open();
    }
}
