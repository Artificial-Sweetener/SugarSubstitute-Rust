// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow the actual registered desktop's deliberate close request without settlement inference.
use super::ParentLifecycle;
pub(crate) use sugarsubstitute_application::supervision::DesktopCloseRequest;

impl ParentLifecycle {
    /// Constrain authenticated cleanup controls to the original role transaction without changing their header.
    pub(super) fn cleanup_control_deadline(
        &self,
        payload: &sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload<'_>,
        supplied: sugarsubstitute_contracts::execution::Moment,
    ) -> sugarsubstitute_contracts::execution::Moment {
        use sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload;
        if matches!(
            payload,
            DecodedPayload::MainRetired(_)
                | DecodedPayload::Release(_)
                | DecodedPayload::ExitIntent { .. }
                | DecodedPayload::ExitCompleted(_)
        ) {
            self.cleanup.map_or(supplied, |budget| supplied.min(budget.settle_until))
        } else {
            supplied
        }
    }
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
        request.exited() || self.final_close_reply_written()
    }
    /// Preserve normal close replies until their original grace expires or their ACK is written.
    /// Global shutdown admission does not authorize cutting this live handshake short.
    pub(crate) fn normal_close_handshake_pending(
        &self,
        now: sugarsubstitute_contracts::execution::Moment,
    ) -> bool {
        self.normal_close_request().is_some_and(|request| {
            now < request.budget().graceful_until && !self.normal_cleanup_ready()
        })
    }
    /// Borrow only this registered role's authenticated whole-content exit transaction.
    /// Historical attempt failures do not cancel an actual later close request; normal-idle
    /// eligibility remains with its stricter independent query.
    pub(crate) fn close_progress_request(&self) -> Option<DesktopCloseRequest> {
        if !self.registered || self.failure.is_some() {
            return None;
        }
        let request = self.exit.as_ref()?.close_progress_request()?;
        (request.run() == self.run
            && request.process().owner == self.run.owner
            && self.cleanup == Some(request.budget()))
        .then_some(request)
    }
    /// Permit only the accepted deliberate role's existing cleanup owners at original grace.
    /// Missing final completion stays unproved; no native exit or normal attribution is inferred.
    pub(crate) fn overdue_close_request(
        &self,
        now: sugarsubstitute_contracts::execution::Moment,
    ) -> Option<DesktopCloseRequest> {
        let request = self.close_progress_request()?;
        (now >= request.budget().graceful_until).then_some(request)
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
            || self.requests.next_reply().is_some() && !self.final_close_reply_written()
    }
    /// After the actual final ACK, retained older replies have no receiving desktop.
    /// Keep their original outcomes and unsent status; they cannot veto physical cleanup.
    /// Queued/in-flight ACKs and other closure paths supply no such delivery proof.
    fn final_close_reply_written(&self) -> bool {
        self.closed
            && self.write.is_none()
            && self
                .close_after_reply
                .is_some_and(|index| self.requests.get(index).is_ok_and(|record| record.sent))
    }
    /// Return only this registered actor's accepted deliberate exit transaction.
    /// The exact process and original budget remain inert facts. Native, backend,
    /// profile and report custody require their existing independent close owners.
    /// Historical acquisition failures remain retained; actual actor failures and unexpected exits deny normal idle.
    pub(crate) fn normal_close_request(&self) -> Option<DesktopCloseRequest> {
        if !self.registered || self.failure.is_some() {
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
