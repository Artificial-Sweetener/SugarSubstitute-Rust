// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enforce original frontend admission before consuming a queued native readiness receipt.
use super::*;
impl ProductionStartup {
    /// Reject only matching current evidence after its original budget expires.
    /// Foreign wrappers or receipt identities continue through the strict policy rejection path.
    /// A queued receipt has no earlier acceptance timestamp and cannot override a reached cutoff.
    pub(super) fn reject_expired_frontend_receipt(
        &mut self,
        observation: &DesktopStartupObservation,
    ) -> Result<bool, Box<dyn Error>> {
        let Some(startup) = &self.startup else {
            return Ok(false);
        };
        let session = startup.session();
        let current = self.policy.identity(session);
        if !matches!(observation, DesktopStartupObservation::FrontendReceipt { identity, receipt }
            if *identity == current && receipt.launch() == current.launch
                && receipt.navigation() == current.navigation)
        {
            return Ok(false);
        }
        self.child.expire_frontend(session.phase())
    }
}
