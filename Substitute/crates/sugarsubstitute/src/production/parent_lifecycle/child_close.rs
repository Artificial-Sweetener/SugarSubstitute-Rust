// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Ask a live desktop to close through its ordinary close when the coordinator closes.
//!
//! A coordinator close of a registered desktop that has not begun its own close sends one
//! Kind25 CloseRequest. The window then closes as the user's close does: close save,
//! ExitIntent, main and lease release, ExitCompleted. The coordinator keeps the pipe open for
//! that handshake and requests role cleanup only once it completed, or once the original
//! shutdown grace expired.
use super::{ParentLifecycle, ParentResources, Result, ShutdownBudget, types::WritePurpose};
use sugarsubstitute_contracts::execution::Moment;
use sugarsubstitute_supervision::lifecycle_protocol::{WireHeader, WirePayload};

/// Whether the close request is still owed, on its way, or delivered.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ChildClose {
    /// The coordinator has not asked.
    #[default]
    Idle,
    /// Asked; the request goes out at the next free write slot until this grace.
    Due(Moment),
    /// The request is being written.
    Writing(Moment),
    /// The child read the request.
    Sent,
}
impl ChildClose {
    /// The request write finished.
    pub(super) fn written(&mut self) {
        if matches!(self, Self::Writing(_)) {
            *self = Self::Sent;
        }
    }
    /// The request write was revoked before issue; it goes out again.
    pub(super) fn revoked(&mut self) {
        if let Self::Writing(until) = *self {
            *self = Self::Due(until);
        }
    }
}

impl ParentLifecycle {
    /// Ask this live desktop to close normally under the coordinator's `global` budget. True
    /// while the coordinator must leave the pipe open for the child's own close: the desktop
    /// is registered, has not failed, exited or begun its close, and the original grace has not
    /// expired. Once the child sent ExitIntent, its accepted close transaction decides.
    pub(crate) fn ask_child_to_close(&mut self, now: Moment, global: ShutdownBudget) -> bool {
        let live = self.registered
            && self.failure.is_none()
            && !self.closed
            && self.cleanup.is_none()
            && self.exit.as_ref().is_none_or(|exit| exit.close_progress_request().is_none());
        if !live || now >= global.graceful_until {
            return false;
        }
        if self.child_close == ChildClose::Idle {
            self.child_close = ChildClose::Due(global.graceful_until);
        }
        true
    }

    /// Whether this desktop was asked to close by the coordinator.
    #[cfg(test)]
    pub(crate) fn child_close(&self) -> ChildClose {
        self.child_close
    }

    /// Write the owed close request when the shared write slot is free.
    pub(super) fn send_child_close(&mut self, resources: &ParentResources<'_, '_>) -> Result<()> {
        let ChildClose::Due(until) = self.child_close else { return Ok(()) };
        if self.write.is_some() || !self.registered || self.closed || self.cleanup.is_some() {
            return Ok(());
        }
        if resources.execution.client.now()? >= until {
            return Ok(());
        }
        let header = WireHeader {
            run: self.run,
            connection: Some(self.connection),
            request: None,
            deadline_qpc: resources.execution.client.encode_counter_deadline(until)?.ticks(),
        };
        if self.write_message(
            resources,
            header,
            WirePayload::CloseRequest,
            until,
            WritePurpose::ChildClose,
        )? {
            self.child_close = ChildClose::Writing(until);
        }
        Ok(())
    }
}
