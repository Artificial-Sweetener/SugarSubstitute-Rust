// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence actual native action delivery across presentation, revision replacement and close.
use super::{ReportRequestId, ReportSurfaceError, ReportSurfaceReceipt};
use std::num::NonZeroU64;
use sugarsubstitute_contracts::incident::{
    DurableAcknowledgement, ExportSelection, ReportAction, ReportBinding, ReportRestartAvailability,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Intent {
    Action(ReportAction),
    Export(ExportSelection),
}
impl Intent {
    /// Native callback and state owner use the same fence; presentation grants no host authority.
    pub(super) const fn allowed(self, restart: ReportRestartAvailability) -> bool {
        !matches!(self, Self::Action(ReportAction::DismissAndRequestRestart))
            || restart.allows_restart()
    }
}
#[derive(Clone, Copy)]
pub(super) struct Request {
    pub(super) id: ReportRequestId,
    pub(super) binding: ReportBinding,
    pub(super) intent: Intent,
}
pub(super) struct State {
    binding: ReportBinding,
    restart: ReportRestartAvailability,
    refreshing: bool,
    presented: bool,
    closing: bool,
    native_retired: bool,
    high_water: u64,
    replacements: u64,
    pending: Option<Request>,
}
impl State {
    pub(super) const fn new(binding: ReportBinding, restart: ReportRestartAvailability) -> Self {
        Self {
            binding,
            restart,
            refreshing: true,
            presented: false,
            closing: false,
            native_retired: false,
            high_water: 0,
            replacements: 0,
            pending: None,
        }
    }
    pub(super) const fn binding(&self) -> ReportBinding {
        self.binding
    }
    pub(super) const fn is_presented(&self) -> bool {
        self.presented
    }
    pub(super) const fn closing(&self) -> bool {
        self.closing
    }
    pub(super) const fn actionable(&self) -> bool {
        self.presented && !self.refreshing && !self.closing && self.pending.is_none()
    }
    pub(super) const fn restart(&self) -> ReportRestartAvailability {
        self.restart
    }
    /// Deny only the exact pending Restart; the original view and other actions remain usable.
    pub(super) fn restart_unavailable(
        &mut self,
        id: ReportRequestId,
        binding: ReportBinding,
    ) -> Result<(), ReportSurfaceError> {
        let pending = self.matching_request(id, binding)?;
        if pending.intent != Intent::Action(ReportAction::DismissAndRequestRestart) {
            return Err(ReportSurfaceError::rejected("report_restart_rejection_action"));
        }
        self.restart = ReportRestartAvailability::Unavailable;
        self.completed_request();
        Ok(())
    }
    pub(super) fn presented(&mut self) -> Result<(), ReportSurfaceError> {
        if self.presented || self.closing {
            return Err(ReportSurfaceError::rejected("report_presented_phase"));
        }
        self.presented = true;
        Ok(())
    }
    pub(super) fn preflight_replace(
        &mut self,
        old: ReportBinding,
        current: ReportBinding,
    ) -> Result<(), ReportSurfaceError> {
        if self.closing
            || !self.presented
            || old != self.binding
            || current.incident() != old.incident()
            || current.session() != old.session()
            || current.revision() <= old.revision()
            || self.pending.is_some()
            || self.replacements == u64::MAX
        {
            return Err(ReportSurfaceError::rejected("report_replace_binding"));
        }
        self.refreshing = true;
        Ok(())
    }
    pub(super) fn replaced(&mut self, current: ReportBinding, restart: ReportRestartAvailability) {
        self.restart = restart;
        self.binding = current;
        self.refreshing = false;
        self.replacements += 1;
    }
    pub(super) fn request(
        &mut self,
        observed: ReportBinding,
        intent: Intent,
    ) -> Result<Request, ReportSurfaceError> {
        if observed != self.binding
            || !self.actionable()
            || !intent.allowed(self.restart)
            || matches!(intent, Intent::Action(ReportAction::Export(_)))
        {
            return Err(ReportSurfaceError::rejected("report_action_fence"));
        }
        let next = self
            .high_water
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .ok_or_else(|| ReportSurfaceError::rejected("report_request_exhausted"))?;
        let request = Request { id: ReportRequestId(next), binding: self.binding, intent };
        self.high_water = next.get();
        self.pending = Some(request);
        Ok(request)
    }
    pub(super) fn matching_request(
        &self,
        id: ReportRequestId,
        binding: ReportBinding,
    ) -> Result<Request, ReportSurfaceError> {
        self.pending
            .filter(|p| {
                p.id == id && p.binding == binding && binding == self.binding && !self.closing
            })
            .ok_or_else(|| ReportSurfaceError::rejected("report_action_result"))
    }
    pub(super) fn completed_request(&mut self) {
        self.pending = None;
    }
    pub(super) fn durable_close(
        &mut self,
        binding: ReportBinding,
        acknowledgement: DurableAcknowledgement,
    ) -> Result<(), ReportSurfaceError> {
        if self.closing
            || binding != self.binding
            || acknowledgement.incident() != binding.incident()
            || acknowledgement.revision() <= binding.revision()
            || !self.pending.is_some_and(|p| {
                matches!(
                    p.intent,
                    Intent::Action(ReportAction::Dismiss | ReportAction::DismissAndRequestRestart)
                )
            })
        {
            return Err(ReportSurfaceError::rejected("report_close_ack"));
        }
        self.closing = true;
        self.refreshing = true;
        Ok(())
    }
    pub(super) fn shutdown(&mut self) {
        self.closing = true;
        self.refreshing = true;
    }
    pub(super) fn native_retired(&mut self) -> Result<(), ReportSurfaceError> {
        if !self.closing || self.native_retired {
            return Err(ReportSurfaceError::rejected("report_native_retirement"));
        }
        self.native_retired = true;
        Ok(())
    }
    pub(super) fn receipt(&self) -> Result<ReportSurfaceReceipt, ReportSurfaceError> {
        if !self.native_retired {
            return Err(ReportSurfaceError::rejected("report_receipt_without_retirement"));
        }
        Ok(ReportSurfaceReceipt {
            session: self.binding.session(),
            presented: self.presented,
            replacements: self.replacements,
            requests: self.high_water,
        })
    }
}
#[cfg(test)]
mod tests;
