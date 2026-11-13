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
    /// Identify discovery I/O before the Launch payload can have reached a coordinator.
    pub(super) fn awaiting_discovery(&self, index: usize) -> bool {
        !self.work.shared.server
            && self.connections[index].pending.is_some_and(|io| {
                io.next == Phase::Challenge
                    && match self.connections[index].phase {
                        Phase::Connecting => matches!(io.role, Role::Handshake | Role::Write),
                        Phase::Challenge => io.role == Role::Read,
                        _ => false,
                    }
            })
    }
    /// Classify only proven discovery loss or an already delivered final response.
    pub(super) fn connection_error(
        &self,
        index: usize,
        error: EntryTransportError,
    ) -> EntryTransportError {
        let unsubmitted = self.awaiting_discovery(index);
        let reply_delivered = self.work.shared.server
            && self.connections[index].phase == Phase::AwaitPeerRetirement
            && self.connections[index].pending.is_some_and(|io| {
                io.role == Role::Read
                    && io.length == 1
                    && io.next == Phase::AwaitPeerRetirement
                    && io.operation.is_none()
                    && io.deadline.is_some()
                    && io.deadline == self.connections[index].peer_retirement_until
            });
        if (unsubmitted || reply_delivered)
            && let EntryTransportError::Native { cause, identity_unrestored: false } = error
        {
            if !cause.contains_owner_panic()
                && matches!(cause.stage(), "pipe_issue" | "pipe_completion")
                && cause.source().is_some_and(peer_closed_pipe)
            {
                return EntryTransportError::PeerDisconnected(cause);
            }
            return EntryTransportError::native(cause);
        }
        error
    }
}

/// Whether a native pipe failure proves the peer closed its end.
///
/// Windows reports ERROR_BROKEN_PIPE, ERROR_NO_DATA or ERROR_PIPE_NOT_CONNECTED.
#[cfg(windows)]
pub(in crate::coordinator_entry) fn peer_closed_pipe(error: &(dyn Error + 'static)) -> bool {
    error.downcast_ref::<windows::core::Error>().is_some_and(|error| {
        [109, 232, 233]
            .into_iter()
            .any(|code| error.code() == windows::core::HRESULT::from_win32(code))
    })
}

/// Whether a native pipe failure proves the peer closed its end.
///
/// Other platforms report a closed pipe peer as a broken pipe.
#[cfg(not(windows))]
pub(in crate::coordinator_entry) fn peer_closed_pipe(error: &(dyn Error + 'static)) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::BrokenPipe)
}
