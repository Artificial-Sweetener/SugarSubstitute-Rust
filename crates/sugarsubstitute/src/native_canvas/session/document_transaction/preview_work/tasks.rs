// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry private candidate construction and reprojection through admitted cooperative tasks.

use super::*;

/// Construct private renderer resources only after actual CPU admission, on its worker.
pub(super) struct PrepareTask {
    task: Option<work::preview::PreparePreview>,
    error: Option<WorkError>,
}
impl PrepareTask {
    /// Preserve a constructor failure until the admitted task delivers its terminal outcome.
    pub(super) fn new(result: Result<work::preview::PreparePreview, WorkError>) -> Self {
        match result {
            Ok(task) => Self { task: Some(task), error: None },
            Err(error) => Self { task: None, error: Some(error) },
        }
    }
}
impl CpuTask<work::preview::ReadyPreview, WorkError> for PrepareTask {
    fn step(
        &mut self,
        context: &JobContext,
    ) -> Result<CpuStep<work::preview::ReadyPreview>, WorkError> {
        if let Some(cause) = self.error.take() {
            return Err(cause);
        }
        self.task
            .as_mut()
            .ok_or_else(|| WorkError::invalid("Preview construction already failed"))?
            .step(context)
    }
}

/// Refit retained candidate pixels under the same original deadline after native resize.
pub(super) struct ReprojectTask {
    pub(super) ready: Option<Box<work::preview::ReadyPreview>>,
    pub(super) target: FrameStamp,
    pub(super) extent: sugarsubstitute_image_ops::Extent,
}
impl CpuTask<work::preview::ReadyPreview, WorkError> for ReprojectTask {
    fn step(
        &mut self,
        context: &JobContext,
    ) -> Result<CpuStep<work::preview::ReadyPreview>, WorkError> {
        let ready = self
            .ready
            .as_mut()
            .ok_or_else(|| WorkError::invalid("Preview refit already completed"))?;
        ready.reproject(self.target, self.extent, context)?;
        Ok(CpuStep::Complete(
            *self.ready.take().ok_or_else(|| WorkError::invalid("Preview refit disappeared"))?,
        ))
    }
}
