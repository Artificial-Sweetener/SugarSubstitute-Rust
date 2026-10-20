// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project shared shutdown budget observations without native transport fields.
/// Describe the first capture or local shutdown measurement transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BudgetStage {
    CapturePending,
    LocalMeasured,
    MeasurementFailed,
    CaptureLatched,
    CaptureMeasurementFailed,
}
/// Observe the original monotonic budget; this record grants no cleanup authority.
pub(crate) struct BudgetTrace {
    pub(crate) stage: BudgetStage,
    pub(crate) moments: Option<[u64; 3]>,
}
impl BudgetTrace {
    pub(crate) fn emit(self) {
        let stage = self.stage;
        match self.moments {
            Some([started_ns, graceful_ns, settle_ns]) => {
                tracing::info!(operation="child_shutdown_budget",stage=?stage,started_ns,graceful_ns,settle_ns)
            }
            None => tracing::info!(operation="child_shutdown_budget",stage=?stage),
        }
    }
}
