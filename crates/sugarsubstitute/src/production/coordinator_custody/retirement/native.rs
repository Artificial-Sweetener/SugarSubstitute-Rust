// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow the original controller, process registry and captured attachment for retirement.
use super::*;
/// Borrow the sole actual native owners without transferring their admission authority.
pub(super) struct Native<'a, 'r> {
    pub(super) parent: BackendCustodyParent,
    pub(super) ticket: &'a mut Option<LaunchTicket>,
    pub(super) attachment: &'a mut attachment::Attachment,
    pub(super) resources: &'a mut BackendResources<'r>,
}
impl Boundary for Native<'_, '_> {
    fn observe(&mut self) -> Result<()> {
        self.parent
            .consume(std::num::NonZeroUsize::new(32).ok_or("Custody drain bound")?)
            .map_err(|cause| diagnostics::failed("profile_retirement_observation", cause))
    }
    fn finish(&mut self, until: Moment) -> Result<bool> {
        self.parent
            .advance_finish(self.resources.client, until)
            .map_err(|cause| diagnostics::failed("finish_exchange", cause))
    }
    fn finish_status(&self) -> BackendCustodyFinishStatus {
        self.parent.finish_status()
    }
    fn retire_family(&mut self, original: ShutdownBudget) -> Result<bool> {
        if let Some(ticket) = *self.ticket {
            let registry =
                self.resources.authority.registry_mut().ok_or("Custody registry absent")?;
            registry.begin_stop(ticket.process(), ShutdownId::new(1)?, original)?;
            let actual = registry.snapshot(ticket.process())?;
            if (actual.cleanup_step_due(self.resources.client.now()?)
                || (actual.settled && registry.custody_retirement_pending(ticket.process())?))
                && !actual.cleanup_step_pending
                && self.resources.client.now()? < original.settle_until
            {
                let watch = if actual.member_watch_required {
                    Some(spec(self.resources, JobClass::NativeObservation, 4096, None)?)
                } else {
                    None
                };
                let cleanup = spec(
                    self.resources,
                    JobClass::InfrastructureLifecycle,
                    4096,
                    Some(original.settle_until),
                )?;
                self.resources
                    .authority
                    .registry_mut()
                    .ok_or("Custody registry absent")?
                    .submit_cleanup_step(
                        self.resources.client,
                        ticket.process(),
                        self.resources.cleanup,
                        cleanup,
                        watch,
                    )?;
            }
            let registry =
                self.resources.authority.registry_mut().ok_or("Custody registry absent")?;
            if registry.snapshot(ticket.process())?.settled
                && !registry.custody_retirement_pending(ticket.process())?
            {
                let actual = registry.snapshot(ticket.process())?;
                tracing::info!(operation="backend_custody_family_retired", key=?self.parent.key(), process=?ticket.process(),
                    forced=actual.forced, root_exit=?actual.root_exit, graceful_until=?original.graceful_until,
                    settle_until=?original.settle_until, "Original helper family retirement facts consumed");
                while registry.take_observation(ticket.process())?.is_some() {}
                registry.retire_settled(ticket.process())?;
                *self.ticket = None;
            }
        }

        Ok(self.ticket.is_none())
    }
    fn close_pipe(&mut self) -> Result<()> {
        self.parent.close().map_err(|cause| diagnostics::failed("pipe_close", cause))
    }
    fn take_terminal(&mut self) -> Result<Option<Terminal>> {
        self.parent.take_terminal().map_err(|cause| diagnostics::failed("pipe_terminal", cause))
    }
    fn parent_retired(&mut self) {
        self.attachment.parent_retired();
    }
}
