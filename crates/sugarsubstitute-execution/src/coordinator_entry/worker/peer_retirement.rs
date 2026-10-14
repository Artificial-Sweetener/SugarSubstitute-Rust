// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve a final reply until actual client close or its already-issued probe cutoff.
use super::*;
use std::error::Error;
impl Active {
    pub(super) fn await_peer_retirement(
        &mut self,
        core: &Core,
        index: usize,
    ) -> Result<(), EntryTransportError> {
        let deadline = self.connections[index]
            .peer_retirement_until
            .ok_or(EntryTransportError::InvalidPhase)?;
        let connection = lock(&self.work.shared.state).connections[index]
            .id
            .ok_or(EntryTransportError::InvalidPhase)?;
        let io = Io {
            role: Role::Read,
            length: 1,
            next: Phase::AwaitPeerRetirement,
            deadline: Some(deadline),
            operation: None,
        };
        self.connections[index].pending = Some(io);
        lock(&self.work.shared.state).connections[index].reading = true;
        let issue = NativeIssue {
            core,
            shared: &self.work.shared,
            context: &self.context,
            index,
            connection,
            role: Role::Read,
            deadline: Some(deadline),
        };
        let lease = self.lease.as_mut().ok_or(EntryTransportError::InvalidPhase)?;
        // The fixed metadata Vec never grows after native admission. This byte stays
        // in its original cell through actual read quiescence, even with the launch loan out.
        let probe = &mut self.connections[index].retirement_probe;
        match lease.read(index, probe, issue)? {
            IssueDispatch::Issued(Progress::Pending) => Ok(()),
            IssueDispatch::Issued(Progress::Complete(bytes)) => {
                self.complete(core, index, io, bytes)
            }
            IssueDispatch::Closed => {
                self.close_connection(core, index, None);
                Ok(())
            }
            IssueDispatch::Expired => self.expire_probe(core, index, core.clock.now()),
        }
    }
    pub(super) fn peer_retirement_error(
        &self,
        index: usize,
        error: EntryTransportError,
    ) -> EntryTransportError {
        if self.work.shared.server
            && self.connections[index].phase == Phase::AwaitPeerRetirement
            && self.connections[index].pending.is_some_and(|io| {
                io.role == Role::Read
                    && io.length == 1
                    && io.next == Phase::AwaitPeerRetirement
                    && io.operation.is_none()
                    && io.deadline.is_some()
                    && io.deadline == self.connections[index].peer_retirement_until
            })
            && let EntryTransportError::Native { cause, identity_unrestored: false } = error
        {
            if !cause.contains_owner_panic()
                && matches!(cause.stage(), "pipe_issue" | "pipe_completion")
                && cause
                    .source()
                    .and_then(|e| e.downcast_ref::<windows::core::Error>())
                    .is_some_and(|e| e.code() == windows::core::HRESULT::from_win32(109))
            {
                return EntryTransportError::PeerDisconnected(cause);
            }
            return EntryTransportError::native(cause);
        }
        error
    }
}
