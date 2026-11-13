// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit finite owned cleanup only at actual completion or immutable stop boundaries.
use super::{Result, evidence::Stop, parent::Parent};
use sugarsubstitute_contracts::{
    execution::{Clock, JobClass},
    lifecycle::{ProcessKey, ShutdownId},
};

impl Parent {
    pub(super) fn ensure_stop(&mut self) -> Result<()> {
        let Some(case) = self.cases.get_mut(self.current) else { return Ok(()) };
        let Some(key) = case.process else { return Ok(()) };
        if case.stop.is_some()
            || case.retired
            || self
                .registry
                .as_ref()
                .ok_or("Actual registry is not prepared")?
                .snapshot(key)?
                .settled
        {
            return Ok(());
        }
        let stop = Stop::new(
            ShutdownId::new(u64::try_from(self.current)? + 1)?,
            self.clock.now(),
            crate::campaign_watch::native_now(),
        )?;
        self.registry.as_mut().ok_or("Actual registry is not prepared")?.begin_stop(
            key,
            stop.id,
            stop.budget,
        )?;
        case.stop = Some(stop);
        case.cleanup_requested = true;
        Ok(())
    }
    pub(super) fn submit_cleanup(&mut self, key: ProcessKey, member_required: bool) -> Result<()> {
        let watch = if member_required {
            Some(self.spec(JobClass::NativeObservation, 4096)?)
        } else {
            None
        };
        let step = self.spec(JobClass::InfrastructureLifecycle, 4096)?;
        // A failed admission is itself failed qualification. Retry only after an
        // actual admitted completion or original grace/total boundary, never cadence.
        self.cases[self.current].cleanup_requested = false;
        let id =
            self.registry.as_mut().ok_or("Actual registry is not prepared")?.submit_cleanup_step(
                self.client.as_ref().ok_or("Lifecycle cleanup client missing")?,
                key,
                self.scope.as_ref().ok_or("Lifecycle cleanup scope missing")?,
                step,
                watch,
            )?;
        self.cases[self.current].cleanup_job = Some(id);
        self.cases[self.current].member_job = watch.map(|spec| spec.id);
        Ok(())
    }
}
