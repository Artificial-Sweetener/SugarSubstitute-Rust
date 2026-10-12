// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact process/token/pipe outputs through checked kernel identity and closure.
use super::{
    CaptureAttachExpectation, CaptureCleanupSnapshot, CaptureEndpointObservation,
    CaptureProcessFacts, ping,
};
use crate::{
    capture_attach::{AttachLease, Shared},
    native::{NativeObservationError, Signal},
    native_pipe::{
        handles::{Handle, adopt},
        security,
    },
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::JobContext;
use windows::{
    Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{Pipes::*, Threading::*},
    },
    core::PCWSTR,
};

#[derive(Clone, Copy)]
pub(super) struct ProcessProof {
    pub(super) facts: CaptureProcessFacts,
    pub(super) identity: security::Identity,
    pub(super) live: bool,
}
pub(super) fn verify_proof(
    expected: CaptureAttachExpectation,
    own: ProcessProof,
    creator: ProcessProof,
    handler: ProcessProof,
    server_pid: u32,
) -> Result<(), NativeObservationError> {
    if own.facts != expected.creator()
        || creator.facts != expected.creator()
        || handler.facts != expected.handler()
        || !own.live
        || !creator.live
        || !handler.live
        || creator.identity != own.identity
        || handler.identity != own.identity
        || ![expected.creator().process_id(), expected.handler().process_id()].contains(&server_pid)
    {
        return Err(ping::invariant("capture_actual_peer_mismatch"));
    }
    Ok(())
}
pub(super) struct Lease {
    expected: CaptureAttachExpectation,
    creator: Handle,
    handler: Handle,
    own_token: Handle,
    creator_token: Handle,
    handler_token: Handle,
    pipe: Handle,
    ping: Option<ping::Ping>,
    ping_complete: bool,
    prepared: bool,
    own: Option<ProcessProof>,
}
impl Lease {
    pub(super) fn new(expected: CaptureAttachExpectation) -> Self {
        Self {
            expected,
            creator: Handle::new(),
            handler: Handle::new(),
            own_token: Handle::new(),
            creator_token: Handle::new(),
            handler_token: Handle::new(),
            pipe: Handle::new(),
            ping: None,
            ping_complete: false,
            prepared: false,
            own: None,
        }
    }
    fn acquire_process(
        slot: &mut Handle,
        facts: CaptureProcessFacts,
    ) -> Result<(), NativeObservationError> {
        // SAFETY: bounded expected PID, only query/synchronize rights; this actual
        // duplicate is inheritable solely for the explicit captured HANDLE_LIST.
        let result = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                true,
                facts.process_id(),
            )
        };
        match result {
            Ok(raw) => adopt(slot, raw, "capture_open_process").map_err(|cause| cause.cause),
            Err(cause) => Err(NativeObservationError::new("capture_open_process", cause)),
        }
    }
    fn proof(
        process: HANDLE,
        custody: &Handle,
        token: &mut Handle,
    ) -> Result<ProcessProof, NativeObservationError> {
        token.close("capture_close_previous_token").map_err(|cause| cause.cause)?;
        let (pid, birth) =
            security::process_facts(process, custody).map_err(|cause| cause.cause)?;
        let facts = CaptureProcessFacts::new(pid, birth)
            .map_err(|cause| NativeObservationError::new("capture_process_facts", cause))?;
        let identity = security::process_identity(process, token).map_err(|cause| cause.cause)?;
        // SAFETY: retained QUERY_LIMITED_INFORMATION/SYNCHRONIZE process handle;
        // current-process pseudo handle is used only for its actual local facts.
        let wait = unsafe { WaitForSingleObject(process, 0) };
        let live = if wait == WAIT_TIMEOUT {
            true
        } else if wait == WAIT_OBJECT_0 {
            false
        } else {
            return Err(NativeObservationError::new(
                "capture_process_live",
                windows::core::Error::from_thread(),
            ));
        };
        Ok(ProcessProof { facts, identity, live })
    }
    fn verify(&mut self) -> Result<(), NativeObservationError> {
        let own = self.own.ok_or_else(|| ping::invariant("capture_current_owner"))?;
        let creator = Self::proof(self.creator.raw(), &self.creator, &mut self.creator_token)?;
        let handler = Self::proof(self.handler.raw(), &self.handler, &mut self.handler_token)?;
        let mut server_pid = 0;
        // SAFETY: exact retained connected local endpoint; no alternate-name lookup.
        unsafe { GetNamedPipeServerProcessId(self.pipe.raw(), &mut server_pid) }
            .map_err(|cause| NativeObservationError::new("capture_pipe_server", cause))?;
        verify_proof(self.expected, own, creator, handler, server_pid)
    }
    fn close_temporary(&mut self) -> Result<bool, NativeObservationError> {
        if let Some(ping) = &mut self.ping
            && !ping.retire(self.pipe.raw())?
        {
            return Ok(false);
        }
        self.ping = None;
        self.pipe.close("capture_close_ping_pipe").map_err(|cause| cause.cause)?;
        self.handler_token.close("capture_close_handler_token").map_err(|cause| cause.cause)?;
        self.creator_token.close("capture_close_creator_token").map_err(|cause| cause.cause)?;
        self.own_token.close("capture_close_owner_token").map_err(|cause| cause.cause)?;
        Ok(true)
    }
}
impl AttachLease for Lease {
    fn prepare(
        &mut self,
        observed: CaptureEndpointObservation,
        context: &JobContext,
        signal: Arc<Signal>,
        shared: &Arc<Shared>,
    ) -> Result<(), NativeObservationError> {
        if observed.expectation() != self.expected {
            return Err(ping::invariant("capture_source_expectation"));
        }
        // SAFETY: bounded current process identity; no handle ownership transfer.
        let current = unsafe { GetCurrentProcess() };
        self.own = Some(Self::proof(current, &Handle::new(), &mut self.own_token)?);
        if self.own.is_none_or(|own| own.facts != self.expected.creator()) {
            return Err(ping::invariant("capture_creator_not_actual_owner"));
        }
        Self::acquire_process(&mut self.creator, self.expected.creator())?;
        Self::acquire_process(&mut self.handler, self.expected.handler())?;
        let mut endpoint = [0u16; 257];
        let value = observed.endpoint_utf16();
        endpoint[..value.len()].copy_from_slice(value);
        const ACCESS: u32 = FILE_READ_DATA.0
            | FILE_WRITE_DATA.0
            | FILE_READ_ATTRIBUTES.0
            | FILE_WRITE_ATTRIBUTES.0
            | SYNCHRONIZE.0;
        // SAFETY: exact source spelling with fixed NUL termination, no retry,
        // read/write/attribute rights and identification-only SQOS. Adopt before setup.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(endpoint.as_ptr()),
                ACCESS,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                None,
            )
        }
        .map_err(|cause| NativeObservationError::new("capture_open_exact_endpoint", cause))?;
        adopt(&mut self.pipe, raw, "capture_open_exact_endpoint").map_err(|cause| cause.cause)?;
        let mode = PIPE_READMODE_MESSAGE;
        // SAFETY: uniquely retained opened message-mode pipe with attribute-write rights.
        unsafe { SetNamedPipeHandleState(self.pipe.raw(), Some(&mode), None, None) }
            .map_err(|cause| NativeObservationError::new("capture_pipe_message_mode", cause))?;
        self.verify()?;
        self.ping = Some(ping::Ping::new(signal));
        self.ping.as_mut().ok_or_else(|| ping::invariant("capture_ping_owner"))?.issue(
            self.pipe.raw(),
            shared,
            context,
        )
    }
    fn progress(&mut self) -> Result<Option<[usize; 2]>, NativeObservationError> {
        if self.prepared {
            return Ok(Some([self.creator.value(), self.handler.value()]));
        }
        if !self.ping_complete {
            if !self
                .ping
                .as_mut()
                .ok_or_else(|| ping::invariant("capture_ping_owner"))?
                .progress(self.pipe.raw())?
            {
                return Ok(None);
            }
            self.ping_complete = true;
            self.verify()?;
        }
        if !self.close_temporary()? {
            return Ok(None);
        }
        self.prepared = true;
        Ok(Some([self.creator.value(), self.handler.value()]))
    }
    fn prepared(&self) -> bool {
        self.prepared
    }
    fn retire(
        &mut self,
        borrowed: bool,
        _: &CaptureCleanupSnapshot,
    ) -> Result<bool, NativeObservationError> {
        if !self.close_temporary()? || borrowed {
            return Ok(false);
        }
        self.handler.close("capture_close_handler_pin").map_err(|cause| cause.cause)?;
        self.creator.close("capture_close_creator_pin").map_err(|cause| cause.cause)?;
        Ok(true)
    }
}
