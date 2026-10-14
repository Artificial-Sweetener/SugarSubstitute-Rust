// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind failed authored close operations to passive retained child/capture diagnostics.

use super::*;
use sugarsubstitute_desktop::{
    StartupCaptureState, StartupCauseCode, StartupCloseFailure, StartupCloseStage,
};

impl ProductionStartup {
    /// Preserve existing first-stage facts while transferring the unchanged original error.
    pub(super) fn close_failure(
        &self,
        stage: StartupCloseStage,
        cause: Box<dyn Error>,
    ) -> Box<dyn Error> {
        let (stage, capture, code) = self.shutdown.failure_diagnostic().unwrap_or_else(|| {
            let (capture, code) = self.child.close_diagnostic(cause.as_ref());
            (stage, capture, code)
        });
        StartupCloseFailure::retain(stage, capture, code, cause)
    }
    /// Tag only an actual error; successful close operations receive no fabricated diagnostics.
    pub(super) fn observed_close<T>(
        &self,
        stage: StartupCloseStage,
        result: Result<T, Box<dyn Error>>,
    ) -> Result<T, Box<dyn Error>> {
        result.map_err(|cause| self.close_failure(stage, cause))
    }
}

/// Project a known retained measurement envelope without walking arbitrary error sources.
pub(super) fn measurement_diagnostic(
    cause: &(dyn Error + 'static),
) -> Option<(StartupCloseStage, StartupCaptureState, StartupCauseCode)> {
    cause
        .downcast_ref::<StartupCloseFailure>()
        .map(|failure| (failure.stage(), failure.capture(), failure.code()))
}
