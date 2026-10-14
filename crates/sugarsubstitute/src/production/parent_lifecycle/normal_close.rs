// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow the actual registered desktop's deliberate close request without settlement inference.
use super::ParentLifecycle;
pub(crate) use sugarsubstitute_application::supervision::DesktopCloseRequest as NormalCloseRequest;

impl ParentLifecycle {
    /// Preserve this role's accepted transaction when global cleanup has an earlier cutoff.
    /// This inert command argument neither renews the global deadline nor admits native cleanup.
    pub(crate) fn cleanup_budget(&self, global: super::ShutdownBudget) -> super::ShutdownBudget {
        self.cleanup.unwrap_or(global)
    }
    /// Keep the existing exact-repeat cleanup admission rule before any native operation.
    pub(super) fn validate_cleanup_budget(
        &self,
        budget: super::ShutdownBudget,
    ) -> Result<(), super::ParentError> {
        if self.cleanup.is_some_and(|original| original != budget) {
            return Err(super::ParentError::ChangedDuplicate);
        }
        Ok(())
    }
    /// Permit per-role cleanup after its completion ACK was actually written,
    /// or after the exact native exit was independently attributed as deliberate.
    /// First intent alone cannot close the pipe or revoke the final reply.
    pub(crate) fn normal_cleanup_ready(&self) -> bool {
        let Some(request) = self.normal_close_request() else { return false };
        if self.normal_close_pending() {
            return false;
        }
        request.exited()
            || (self.closed
                && self.write.is_none()
                && self
                    .close_after_reply
                    .is_some_and(|index| self.requests.get(index).is_ok_and(|record| record.sent)))
    }
    /// Pending authored work prevents normal idle; original transport/native custody still settles during close.
    fn normal_close_pending(&self) -> bool {
        self.profile_pending.is_some()
            || self.main_retirement.is_some()
            || self.attempt_cleanup.is_some()
            || self.output.is_some()
            || self.forwarded.is_some()
            || self.acquisition_receive.is_some()
            || self.rejected_frame.is_some()
            || self.write.is_some()
            || self.requests.next_reply().is_some()
    }
    /// Return only this registered actor's accepted deliberate exit transaction.
    /// The exact process and original budget remain inert facts. Native, backend,
    /// profile and report custody require their existing independent close owners.
    /// Failed startup, unexpected exit and original actor failures cannot request normal idle.
    pub(crate) fn normal_close_request(&self) -> Option<NormalCloseRequest> {
        if !self.registered || self.failure.is_some() || self._attempt_failure.is_some() {
            return None;
        }
        let request = self.exit.as_ref()?.normal_close_request()?;
        if request.run() != self.run
            || request.process().owner != self.run.owner
            || self.cleanup != Some(request.budget())
        {
            return None;
        }
        Some(request)
    }
}
