// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the actual recovery handler/database Job before owner-loss containment.
use super::*;
impl ProfileRegistration {
    /// Select capture only for the authenticated recovery purpose after actual root adoption.
    pub(in crate::native_claims::backend_custody) fn owner_lost(
        &mut self,
        until: Moment,
    ) -> Result<(), ClaimError> {
        if self.cleaning || self.capture_wait || self.settled {
            return Ok(());
        }
        if self.recovery_grant.is_some() && self.root_adopted {
            let until = self
                .slot
                .turn()?
                .begin_recovery_capture()
                .map_err(|cause| ClaimError::new("recovery_profile_capture_cutoff", cause))?;
            self.recovery_settle_until =
                Some(until.checked_add(std::time::Duration::from_secs(15)).map_err(|cause| {
                    ClaimError::new("recovery_profile_containment_cutoff", cause)
                })?);
            self.capture_wait = true;
            return Ok(());
        }
        self.stop(until)
    }
    /// Timer delivery alone authorizes no close; the same occupied turn checks its source.
    pub(in crate::native_claims::backend_custody) fn advance_owner_loss(
        &mut self,
    ) -> Result<(), ClaimError> {
        if !self.capture_wait {
            return Ok(());
        }
        if self
            .slot
            .turn()?
            .recovery_capture_pending()
            .map_err(|cause| ClaimError::new("recovery_profile_capture_observation", cause))?
        {
            return Ok(());
        }
        self.stop(Moment::from_nanos(u64::MAX))?;
        self.recovery_settle_until = Some(self.slot.cleanup_proof()?.until());
        self.capture_wait = false;
        Ok(())
    }
    pub(in crate::native_claims::backend_custody) const fn capture_pending(&self) -> bool {
        self.capture_wait
    }
    pub(in crate::native_claims::backend_custody) const fn recovery_settle_until(
        &self,
    ) -> Option<Moment> {
        self.recovery_settle_until
    }
}
