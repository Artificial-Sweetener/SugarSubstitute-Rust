// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence final native acceptance against the exact connection, original deadline and close command.
use super::*;
use sugarsubstitute_contracts::coordinator_entry::EntryConnectionId;
pub(crate) enum IssueDispatch<T> {
    Issued(T),
    Closed,
    Expired,
}
pub(crate) struct NativeIssue<'a> {
    pub(crate) core: &'a Core,
    pub(super) shared: &'a Shared,
    pub(crate) context: &'a JobContext,
    pub(crate) index: usize,
    pub(crate) connection: EntryConnectionId,
    pub(crate) role: crate::native_deadlines::Role,
    pub(crate) deadline: Option<Moment>,
}
impl NativeIssue<'_> {
    pub(crate) fn execute<T>(
        self,
        native: impl FnOnce() -> Result<T, EntryTransportError>,
    ) -> Result<IssueDispatch<T>, EntryTransportError> {
        let at = match self.core.clock.observe_unlatched() {
            Ok(at) => at,
            Err(payload) => self.core.clock.fail_observation(payload),
        };
        let state = lock(&self.core.state);
        let stopping = state.stopping;
        let record = state.jobs.get(&self.shared.job_id);
        let current = self.context.owner_id() == self.core.id
            && record.is_some_and(|record| {
                !record.retiring
                    && !matches!(
                        record.phase,
                        sugarsubstitute_contracts::execution::JobPhase::Terminal(_)
                    )
                    && record.scope_id == self.context.scope_id()
                    && record.spec.id == self.context.job_id()
            });
        let stopped =
            record.and_then(|record| record.native_deadlines).is_none_or(|slots| slots.stopped());
        let policy_deadline = record.and_then(|record| record.deadline);
        let entry = lock(&self.shared.state);
        drop(state);
        let slot =
            entry.connections.get(self.index).ok_or(EntryTransportError::InvalidOperation)?;
        if !current
            || stopping
            || stopped
            || entry.stopped
            || slot.id != Some(self.connection)
            || slot.closing
            || match self.role {
                crate::native_deadlines::Role::Handshake => false,
                crate::native_deadlines::Role::Read => !slot.reading,
                crate::native_deadlines::Role::Write => !slot.writing,
            }
            || self.context.cancellation_requested()
        {
            return Ok(IssueDispatch::Closed);
        }
        if self.core.clock.failed() || self.context.cancellation_requested() {
            return Ok(IssueDispatch::Closed);
        }
        if policy_deadline.is_some_and(|deadline| at >= deadline)
            || self.deadline.is_some_and(|deadline| at >= deadline)
            || slot.until.is_some_and(|until| at >= until)
            || ((!self.shared.server || entry.owner.is_none())
                && at >= self.shared.original_preparation)
        {
            return Ok(IssueDispatch::Expired);
        }
        // Only nonblocking native acceptance/adoption runs while the session close fence is held.
        let accepted = native()?;
        drop(entry);
        Ok(IssueDispatch::Issued(accepted))
    }
}
