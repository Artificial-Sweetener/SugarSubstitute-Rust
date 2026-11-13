// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Report actual child registration without granting handle or retirement authority.

use sugarsubstitute_contracts::{
    crash::CaptureTarget,
    execution::{ExecutionId, JobId},
};

/// Historical genuine native registration, published under fresh original eligibility.
/// Construction remains with the actual worker's final Core terminal fence.
/// This value proves neither temporary settlement nor process-exit retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesktopCaptureAttached {
    pub(super) target: CaptureTarget,
    pub(super) execution: ExecutionId,
    pub(super) job: JobId,
}

impl DesktopCaptureAttached {
    /// Observe the exact capture target registered by the original admitted child job.
    pub const fn target(self) -> CaptureTarget {
        self.target
    }

    /// Observe the real execution owner identity; this grants no admission authority.
    pub const fn execution(self) -> ExecutionId {
        self.execution
    }

    /// Observe the original admitted job; this creates no lifetime or close proof.
    pub const fn job(self) -> JobId {
        self.job
    }
}
