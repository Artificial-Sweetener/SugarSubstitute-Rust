// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the original native capture alive across the unlocked qualification effect.
use super::RegisteredDesktopFaultError;
use crate::{
    admission::Core,
    desktop_capture::{control::AcceptedCaptureStage, metadata::DesktopCaptureCustodyRef},
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::{execution::*, lifecycle::RunKey};

/// Exact bounded factory storage is charged before its box enters the existing worker.
pub(super) const WORK_BYTES: u64 = (size_of::<Worker>() + 256) as u64;
/// Final native source/header disposal precedes the actual finite storage claim.
pub(super) struct Worker {
    pub(super) source: DesktopCaptureCustodyRef,
    pub(super) core: Arc<Core>,
    pub(super) run: RunKey,
    pub(super) _storage: MemoryClaim,
}
/// Constructor-private borrowed effect fence on the actual capture's accepted slot.
/// Stage zero is reserved to this owner; native protocol stages start at one.
pub(super) struct Armed<'a> {
    worker: &'a Worker,
    context: JobContext,
}
impl Worker {
    /// Execute synchronously on the actual admitted worker; no supplied callback escapes.
    pub(super) fn run(self, context: JobContext) -> Result<(), RegisteredDesktopFaultError> {
        let armed = self.arm(context).map_err(RegisteredDesktopFaultError::Stopped)?;
        #[cfg(all(windows, target_arch = "x86_64"))]
        {
            super::windows_trap::run(&armed)
        }
        #[cfg(not(all(windows, target_arch = "x86_64")))]
        {
            drop(armed);
            Err(RegisteredDesktopFaultError::Platform)
        }
    }
    fn arm(&self, context: JobContext) -> Result<Armed<'_>, AdmissionError> {
        let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&self.core.state);
        crate::capture_bundle::validate::live(&self.core, &state, &context, now)?;
        super::admission::source(
            &self.core,
            &state,
            context.scope_id(),
            &self.source,
            self.run,
            now,
        )?;
        let mut stage = lock(&self.source.state);
        if stage.fault_job != Some(context.job_id()) || stage.accepted_stage.is_some() {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        stage.accepted_stage =
            Some(AcceptedCaptureStage { attach: 0, stage: 0, nonce: context.job_id().get() });
        Ok(Armed { worker: self, context })
    }
}
impl Armed<'_> {
    /// Recheck actual Core clock/cancellation and the same physical fence before each fresh effect.
    #[cfg(all(windows, target_arch = "x86_64"))]
    pub(super) fn check(&self) -> Result<(), RegisteredDesktopFaultError> {
        let result = (|| {
            let now = self.worker.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
            let state = lock(&self.worker.core.state);
            crate::capture_bundle::validate::live(&self.worker.core, &state, &self.context, now)?;
            super::admission::source(
                &self.worker.core,
                &state,
                self.context.scope_id(),
                &self.worker.source,
                self.worker.run,
                now,
            )?;
            let stage = lock(&self.worker.source.state);
            if stage.fault_job != Some(self.context.job_id())
                || !stage.accepted_stage.is_some_and(|slot| {
                    slot.attach == 0 && slot.stage == 0 && slot.nonce == self.context.job_id().get()
                })
            {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            Ok(())
        })();
        result.map_err(RegisteredDesktopFaultError::Stopped)
    }
}
impl Drop for Armed<'_> {
    fn drop(&mut self) {
        let core = &self.worker.core;
        let state = lock(&core.state);
        let mut stage = lock(&self.worker.source.state);
        if stage.accepted_stage.is_some_and(|slot| {
            slot.attach == 0 && slot.stage == 0 && slot.nonce == self.context.job_id().get()
        }) {
            stage.accepted_stage = None;
        }
        drop(stage);
        drop(state);
        core.changed();
    }
}
