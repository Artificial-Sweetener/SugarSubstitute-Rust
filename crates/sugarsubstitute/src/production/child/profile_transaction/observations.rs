// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt original bounded desktop observations only at their authenticated prerequisite stage.
use super::*;
use sugarsubstitute_desktop::DesktopStartupObservation;
impl ProfileTransaction {
    /// Borrow the caller's original desktop observation slot. True transfers accepted facts;
    /// false leaves unrelated observations for their owner. Every error leaves the slot intact.
    /// This method grants no authorization; the next matching coordinator reply does that.
    /// # Errors
    /// Rejects cancellation, wrong attempt/order or invalid observed PID without losing custody.
    pub(in super::super) fn observe(
        &mut self,
        observation: &mut Option<DesktopStartupObservation>,
    ) -> Result<bool, ProfileError> {
        let eligible = match observation.as_ref() {
            Some(DesktopStartupObservation::EnvironmentPrepared { identity, .. }) => {
                *identity == self.identity()
                    && self.progress.phase == Phase::EnvironmentWaiting
                    && self.environment.is_none()
            }
            Some(DesktopStartupObservation::ControllerPrepared {
                identity,
                browser_process_id,
            }) => {
                *identity == self.identity()
                    && self.progress.phase == Phase::ControllerWaiting
                    && *browser_process_id != 0
                    && self.browser.is_none()
            }
            _ => return Ok(false),
        };
        self.check()?;
        if !eligible {
            return Err(ProfileError::Order);
        }
        match observation.take().ok_or(ProfileError::Order)? {
            DesktopStartupObservation::EnvironmentPrepared { facts, .. } => {
                self.progress.total = facts.as_utf16().len();
                self.progress.offset = 0;
                self.environment = Some(facts);
                self.progress.phase = Phase::Environment;
            }
            DesktopStartupObservation::ControllerPrepared { browser_process_id, .. } => {
                self.browser = NonZeroU32::new(browser_process_id);
                self.progress.phase = Phase::Browser;
            }
            other => {
                *observation = Some(other);
                return Err(ProfileError::Order);
            }
        }
        Ok(true)
    }
}
