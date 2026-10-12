// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recheck actual kernel PID, birth, liveness and primary/message identity from retained pins.
use super::*;
impl Lease {
    pub(super) fn verified_kernel(&mut self) -> Result<VerifiedPeer, LifecyclePipeFailure> {
        let data = self.data.as_ref().ok_or_else(|| failure("pipe_bootstrap_owner"))?;
        let mut pid = 0;
        let mut session = 0;
        if self.server {
            if !self.read_completed {
                return Err(failure("pipe_authenticate_before_read"));
            }
            self.pipe.check("pipe_client_pid")?;
            // SAFETY: The live owned connected server pipe and fixed output are valid.
            unsafe { GetNamedPipeClientProcessId(self.pipe.raw(), &mut pid) }
                .map_err(|e| error("pipe_client_pid", e))?;
            self.pipe.check("pipe_client_pid")?;
            self.pipe.check("pipe_client_session")?;
            // SAFETY: The same connected server pipe owns this bounded session query.
            unsafe { GetNamedPipeClientSessionId(self.pipe.raw(), &mut session) }
                .map_err(|e| error("pipe_client_session", e))?;
            self.pipe.check("pipe_client_session")?;
        } else {
            self.pipe.check("pipe_server_pid")?;
            // SAFETY: The live owned client pipe and fixed output are valid.
            unsafe { GetNamedPipeServerProcessId(self.pipe.raw(), &mut pid) }
                .map_err(|e| error("pipe_server_pid", e))?;
            self.pipe.check("pipe_server_pid")?;
            self.pipe.check("pipe_server_session")?;
            // SAFETY: The same connected client pipe owns this bounded session query.
            unsafe { GetNamedPipeServerSessionId(self.pipe.raw(), &mut session) }
                .map_err(|e| error("pipe_server_session", e))?;
            self.pipe.check("pipe_server_session")?;
        }
        let expected = if self.server {
            security::process_facts(self.input.raw(), &self.input)?
        } else {
            (data.pid, data.birth)
        };
        peer::address(pid, session, expected, data.session)?;
        self.observed_token.close_normal("pipe_close_previous_peer_token")?;
        self.observed_process.close_normal("pipe_close_previous_peer_process")?;
        self.peer_token.close_normal("pipe_close_previous_enrollment_token")?;
        self.observed_process.reserve("pipe_open_actual_peer")?;
        // SAFETY: Query and zero-duration synchronization access only; PID is kernel-reported.
        let raw = unsafe {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE, false, pid)
        }
        .map_err(|e| error("pipe_open_actual_peer", e))?;
        adopt(&mut self.observed_process, raw, "pipe_open_actual_peer")?;
        peer::birth(
            security::process_facts(self.observed_process.raw(), &self.observed_process)?,
            expected,
        )?;
        let identity = self.identity.ok_or_else(|| failure("pipe_identity_owner"))?;
        let actual =
            security::process_identity(self.observed_process.raw(), &mut self.observed_token)?;
        peer::identity(actual, identity, "pipe_peer_account_logon_session_mismatch")?;
        if self.server {
            peer::identity(
                security::process_identity(self.input.raw(), &mut self.peer_token)?,
                identity,
                "pipe_enrolled_account_mismatch",
            )?;
        }
        self.observed_process.check("pipe_peer_liveness")?;
        peer::live(self.observed_process.raw())?;
        self.observed_process.check("pipe_peer_liveness")?;

        Ok(VerifiedPeer {
            binding: data.binding,
            process: self.process,
            pid,
            birth: expected.1,
            session,
        })
    }
}
