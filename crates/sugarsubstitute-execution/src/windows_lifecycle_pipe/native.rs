// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Confine overlapped pipes, bootstrap sections and credential inspection to native ownership.
use crate::{
    lifecycle_pipe::*,
    native::{NativeObservationError, Signal},
    native_deadlines::Role,
};
use std::{os::windows::io::IntoRawHandle, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, Moment};
use sugarsubstitute_contracts::lifecycle::{ProcessKey, RunKey};
use windows::{
    Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{IO::*, Pipes::*, Threading::*},
    },
    core::PCWSTR,
};
#[path = "bootstrap.rs"]
mod bootstrap;
#[path = "counter.rs"]
pub(crate) mod counter;
#[path = "handles.rs"]
mod handles;
#[path = "overlapped.rs"]
mod overlapped;
#[path = "peer.rs"]
mod peer;
#[path = "prepare.rs"]
mod prepare;
#[path = "security.rs"]
mod security;
use handles::*;
// Message-mode setup needs attribute-write access; generic write would also
// grant FILE_CREATE_PIPE_INSTANCE through the append-data bit.
pub(super) const CLIENT_ACCESS: u32 = FILE_READ_DATA.0
    | FILE_WRITE_DATA.0
    | FILE_READ_ATTRIBUTES.0
    | FILE_WRITE_ATTRIBUTES.0
    | SYNCHRONIZE.0;
pub(super) struct Lease {
    server: bool,
    run: Option<RunKey>,
    original: Option<LifecyclePipeDeadline>,
    data: Option<bootstrap::Data>,
    input: Handle,
    process: Option<ProcessKey>,
    pipe: Handle,
    section: bootstrap::Section,
    own_token: Handle,
    peer_token: Handle,
    observed_token: Handle,
    message_token: Handle,
    observed_process: Handle,
    identity: Option<security::Identity>,
    operations: [Option<overlapped::Operation>; 3],
    read_completed: bool,
}
impl Lease {
    fn new(server: bool, run: Option<RunKey>, original: Option<LifecyclePipeDeadline>) -> Self {
        Self {
            server,
            run,
            original,
            data: None,
            input: Handle(0),
            process: None,
            pipe: Handle(0),
            section: bootstrap::Section::new(),
            own_token: Handle(0),
            peer_token: Handle(0),
            observed_token: Handle(0),
            message_token: Handle(0),
            observed_process: Handle(0),
            identity: None,
            operations: std::array::from_fn(|_| None),
            read_completed: false,
        }
    }
    pub(super) fn server(run: RunKey) -> Self {
        Self::new(true, Some(run), None)
    }
    pub(super) fn client(original: LifecyclePipeDeadline) -> Self {
        Self::new(false, None, Some(original))
    }
    fn operation(
        &mut self,
        role: Role,
    ) -> Result<&mut overlapped::Operation, LifecyclePipeFailure> {
        self.operations[role.index()].as_mut().ok_or_else(|| failure("pipe_operation_owner"))
    }
}
impl PipeLease for Lease {
    fn adopt(&mut self, input: PeerInput) {
        match input {
            PeerInput::Owned { handle, process } => {
                self.process = process;
                self.input.0 = handle.into_raw_handle() as usize;
            }
            PeerInput::Inherited { section } => self.input.0 = section,
            #[cfg(test)]
            PeerInput::Simulated => {}
        }
    }
    fn prepare(
        &mut self,
        signals: [Arc<Signal>; 3],
        context: &JobContext,
        handshake: Moment,
    ) -> Result<usize, LifecyclePipeFailure> {
        for (slot, signal) in self.operations.iter_mut().zip(signals) {
            *slot = Some(overlapped::Operation::new(signal));
        }
        if self.server { self.prepare_server(context, handshake) } else { self.prepare_client() }
    }
    fn run(&self) -> Option<RunKey> {
        self.run
    }
    fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        self.data.as_ref().map(|data| data.deadline)
    }
    fn capability(&self) -> Option<[u8; 32]> {
        if self.server { None } else { self.data.as_ref().map(|data| data.capability) }
    }
    fn connect(&mut self) -> Result<Progress, LifecyclePipeFailure> {
        if !self.server {
            return Ok(Progress::Complete(0));
        }
        let pipe = self.pipe.raw();
        let operation = self.operation(Role::Handshake)?;
        operation.prepare()?;
        // SAFETY: operation storage/event/callback are owned before Issue and remain retained.
        let result = unsafe { ConnectNamedPipe(pipe, Some(operation.pointer())) };
        let accepted = operation.accept(result, true)?;
        operation.initial_progress(accepted, pipe)
    }
    fn read(&mut self, bytes: &mut [u8]) -> Result<Progress, LifecyclePipeFailure> {
        let pipe = self.pipe.raw();
        let operation = self.operation(Role::Read)?;
        operation.prepare()?;
        // SAFETY: admitted fixed buffer remains owned by Active until completion and quiescence.
        let result = unsafe { ReadFile(pipe, Some(bytes), None, Some(operation.pointer())) };
        let accepted = operation.accept(result, false)?;
        let result = operation.initial_progress(accepted, pipe)?;
        if matches!(result, Progress::Complete(_)) {
            self.read_completed = true;
        }
        Ok(result)
    }
    fn write(
        &mut self,
        bytes: &[u8],
        issue: WriteIssue<'_>,
    ) -> Result<WriteDispatch<Progress>, LifecyclePipeFailure> {
        let pipe = self.pipe.raw();
        let operation = self.operation(Role::Write)?;
        operation.prepare()?;
        match issue.execute(|| {
            // SAFETY: exact immutable buffer and prepared OVERLAPPED stay retained through settlement.
            let result = unsafe { WriteFile(pipe, Some(bytes), None, Some(operation.pointer())) };
            operation.accept(result, false)
        })? {
            WriteDispatch::Issued(accepted) => {
                Ok(WriteDispatch::Issued(operation.initial_progress(accepted, pipe)?))
            }
            WriteDispatch::Revoked => Ok(WriteDispatch::Revoked),
            WriteDispatch::Stopped => Ok(WriteDispatch::Stopped),
            WriteDispatch::Expired => Ok(WriteDispatch::Expired),
        }
    }
    fn poll(&mut self, role: Role) -> Result<Progress, LifecyclePipeFailure> {
        let pipe = self.pipe.raw();
        let result = self.operation(role)?.poll(pipe)?;
        if role == Role::Read && matches!(result, Progress::Complete(_)) {
            self.read_completed = true;
        }
        Ok(result)
    }
    fn quiesce(&mut self, role: Role) -> Result<(), LifecyclePipeFailure> {
        self.operation(role)?.quiesce()
    }
    fn verify_message(&mut self) -> Result<(), LifecyclePipeFailure> {
        if !self.server {
            return Ok(());
        }
        if !self.read_completed {
            return Err(failure("pipe_verify_before_read"));
        }
        let identity = self.identity.ok_or_else(|| failure("pipe_identity_owner"))?;
        // The previous token remains owned until this checked close succeeds.
        self.message_token.close("pipe_close_previous_message_token")?;
        let actual = security::message_identity(self.pipe.raw(), &mut self.message_token)?;
        peer::identity(actual, identity, "pipe_message_identity_mismatch")
    }
    fn authenticate(
        &mut self,
        candidate: &[u8; 32],
    ) -> Result<VerifiedLifecyclePeer, LifecyclePipeFailure> {
        let data = self.data.as_ref().ok_or_else(|| failure("pipe_bootstrap_owner"))?;
        peer::capability(candidate, &data.capability)?;
        let mut pid = 0;
        let mut session = 0;
        if self.server {
            if !self.read_completed {
                return Err(failure("pipe_authenticate_before_read"));
            }
            // SAFETY: The live owned connected server pipe and fixed output are valid.
            unsafe { GetNamedPipeClientProcessId(self.pipe.raw(), &mut pid) }
                .map_err(|e| error("pipe_client_pid", e))?;
            // SAFETY: The same connected server pipe owns this bounded session query.
            unsafe { GetNamedPipeClientSessionId(self.pipe.raw(), &mut session) }
                .map_err(|e| error("pipe_client_session", e))?;
        } else {
            // SAFETY: The live owned client pipe and fixed output are valid.
            unsafe { GetNamedPipeServerProcessId(self.pipe.raw(), &mut pid) }
                .map_err(|e| error("pipe_server_pid", e))?;
            // SAFETY: The same connected client pipe owns this bounded session query.
            unsafe { GetNamedPipeServerSessionId(self.pipe.raw(), &mut session) }
                .map_err(|e| error("pipe_server_session", e))?;
        }
        let expected = if self.server {
            security::process_facts(self.input.raw())?
        } else {
            (data.pid, data.birth)
        };
        peer::address(pid, session, expected, data.session)?;
        // SAFETY: Only query access is requested for the actual kernel-reported peer PID.
        let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map_err(|e| error("pipe_open_actual_peer", e))?;
        adopt(&mut self.observed_process, raw, "pipe_open_actual_peer")?;
        peer::birth(security::process_facts(self.observed_process.raw())?, expected)?;
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
        Ok(VerifiedLifecyclePeer {
            run: data.run,
            process: self.process,
            pid,
            birth: expected.1,
            session,
        })
    }
    fn take_completed_write(&mut self) -> Option<u32> {
        self.operations[Role::Write.index()]
            .as_mut()
            .and_then(overlapped::Operation::take_completed)
    }
    fn retire(&mut self, pinned: bool) -> Result<bool, LifecyclePipeFailure> {
        let pipe = self.pipe.raw();
        let mut settled = true;
        let mut failure: Option<LifecyclePipeFailure> = None;
        for operation in self.operations.iter_mut().flatten() {
            match operation.retire(pipe) {
                Ok(retired) => settled &= retired,
                Err(error) => {
                    settled = false;
                    if let Some(first) = &mut failure {
                        first.identity_unrestored |= error.identity_unrestored;
                        first.cause.retain_cleanup(error.cause);
                    } else {
                        failure = Some(error);
                    }
                }
            }
        }
        if let Some(failure) = failure {
            return Err(failure);
        }
        if !settled {
            return Ok(false);
        }
        self.pipe.close("pipe_close_connection")?;
        if !self.section.retire(pinned)? {
            return Ok(false);
        }
        for handle in [
            &mut self.message_token,
            &mut self.peer_token,
            &mut self.observed_token,
            &mut self.own_token,
            &mut self.observed_process,
            &mut self.input,
        ] {
            handle.close("pipe_close_security_handle")?;
        }
        if let Some(data) = &mut self.data {
            data.capability.fill(0);
        }
        Ok(true)
    }
}
