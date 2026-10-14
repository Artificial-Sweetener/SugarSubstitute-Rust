// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the actual readiness owner before the existing exact Main retirement.
use super::*;
impl MainActor {
    /// Validate identity before borrowing passive facts; repeated close emits no second failure.
    pub(crate) fn observe_retirement_reason(
        &self,
        identity: crate::MainIdentity,
        reason: crate::MainRetirementReason,
    ) -> Result<(), DesktopError> {
        if identity != self.identity() {
            return Err(DesktopError::message("retire_main", "main identity differs"));
        }
        if reason == crate::MainRetirementReason::FrontendTimeout
            && self.admission.timeout_diagnostic_allowed(identity)?
        {
            let owner = self
                .readiness
                .as_ref()
                .ok_or_else(|| DesktopError::message("main_readiness", "readiness retired"))?;
            DesktopError::observe_frontend_timeout(identity, owner.wait_snapshot());
        }
        Ok(())
    }
}
