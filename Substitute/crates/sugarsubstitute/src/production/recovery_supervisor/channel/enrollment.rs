// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer the genuine Root received frame through the execution-owned resume boundary.
use super::*;
use sugarsubstitute_execution::recovery_supervisor::{
    AuthenticatedRecoveryOrigin, RecoveryDesktopEnrollment,
};
impl Channel {
    /// Mint one nonclone native resume permission from this exact session's actual frame.
    /// Wrong command remains retained for the control owner; rejection preserves its guard.
    pub(in crate::production::recovery_supervisor) fn take_enrollment(
        &mut self,
        origin: &AuthenticatedRecoveryOrigin,
        expected: RecoveryProcessIdentity,
    ) -> Result<Option<RecoveryDesktopEnrollment>> {
        if !self.startup_transferred || self.closing {
            return Err("Recovery enrollment channel unavailable".into());
        }
        let Some(frame) = self.received.as_ref() else {
            return Ok(None);
        };
        if !matches!(
            decode(frame.as_bytes(), RecoveryReceiver::Supervisor)?.command,
            RecoveryCommand::DesktopEnrolled(_)
        ) {
            return Ok(None);
        }
        let permit = origin.admit_desktop_enrollment(frame, expected)?;
        self.received = None;
        Ok(Some(permit))
    }
}
