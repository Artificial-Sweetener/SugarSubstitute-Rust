// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain every entry pipe, operation and partial credential handle on the sole native worker.
use super::security::{self, PeerPin};
use crate::{
    coordinator_entry::{
        EntryLease, EntryTransportError, IssueDispatch, KernelPeer, MAX_ENTRY_CONNECTIONS,
        MAX_ENTRY_FRAME_BYTES, NativeIssue, Progress,
    },
    native::{NativeObservationError, Signal},
    native_deadlines::Role,
    native_pipe::{
        self,
        handles::{Handle, adopt},
        overlapped::Operation,
        security::{Descriptor, Identity},
    },
};
use std::{error::Error, sync::Arc};
use sugarsubstitute_contracts::{
    coordinator_entry::EntryNativeIdentityFacts,
    execution::{JobContext, Moment},
};
use windows::{
    Win32::{Security::Cryptography::*, Storage::FileSystem::*, System::Pipes::*},
    core::PCWSTR,
};

mod slot;
use slot::Slot;
pub(super) struct Lease {
    server: bool,
    expected: EntryNativeIdentityFacts,
    slots: Vec<Slot>,
    own_token: Handle,
    own_image: Handle,
    identity: Option<Identity>,
}
impl Lease {
    pub(super) fn new(server: bool, expected: EntryNativeIdentityFacts) -> Self {
        Self {
            server,
            expected,
            slots: Vec::new(),
            own_token: Handle::new(),
            own_image: Handle::new(),
            identity: None,
        }
    }
    fn slot(&mut self, index: usize) -> Result<&mut Slot, EntryTransportError> {
        self.slots.get_mut(index).ok_or(EntryTransportError::InvalidOperation)
    }
}
impl EntryLease for Lease {
    fn sample_counter(&mut self) -> Result<(u64, u64), NativeObservationError> {
        crate::windows_counter::sample()
    }
    fn prepare(
        &mut self,
        signals: &[[Arc<Signal>; 3]],
        context: &JobContext,
        original_preparation: Moment,
    ) -> Result<(), EntryTransportError> {
        let count = if self.server { MAX_ENTRY_CONNECTIONS } else { 1 };
        if signals.len() != count || !self.slots.is_empty() {
            return Err(EntryTransportError::InvalidPhase);
        }
        self.slots = (0..count).map(|_| Slot::new()).collect();
        let own = security::own_identity(&self.expected, &mut self.own_token, &mut self.own_image)?;
        self.identity = Some(own);
        let name = endpoint(self.expected.namespace());
        let mut descriptor = Descriptor::new(own)?;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if context.cancellation_requested() {
                return Err(EntryTransportError::Closed);
            }
            if context.now() >= original_preparation {
                return Err(EntryTransportError::InvalidDeadline);
            }
            slot.signals = Some(signals[index].clone());
            slot.operations =
                std::array::from_fn(|role| Some(Operation::new(signals[index][role].clone())));
            if self.server {
                let flags = PIPE_ACCESS_DUPLEX
                    | FILE_FLAG_OVERLAPPED
                    | if index == 0 {
                        FILE_FLAG_FIRST_PIPE_INSTANCE
                    } else {
                        FILE_FLAGS_AND_ATTRIBUTES(0)
                    };
                let attributes = descriptor.attributes();
                // SAFETY: literal derived local endpoint, exact fixed count/buffers and bounded explicit ACL.
                let raw = unsafe {
                    CreateNamedPipeW(
                        PCWSTR(name.as_ptr()),
                        flags,
                        PIPE_TYPE_MESSAGE
                            | PIPE_READMODE_MESSAGE
                            | PIPE_WAIT
                            | PIPE_REJECT_REMOTE_CLIENTS,
                        MAX_ENTRY_CONNECTIONS as u32,
                        MAX_ENTRY_FRAME_BYTES as u32,
                        MAX_ENTRY_FRAME_BYTES as u32,
                        0,
                        Some(&attributes),
                    )
                };
                adopt(&mut slot.pipe, raw, "entry_create_server")?;
            }
        }
        Ok(())
    }
    fn connect(
        &mut self,
        index: usize,
        issue: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        if !self.server {
            let name = endpoint(self.expected.namespace());
            let slot = self.slot(index)?;
            return issue.execute(|| {
                if slot.pipe.value() == 0 {
                    open_client(slot, &name)?;
                }
                slot.connected = true;
                Ok(Progress::Complete(0))
            });
        }
        let slot = self.slot(index)?;
        let pipe = slot.pipe.raw();
        let operation = slot.operation(Role::Handshake)?;
        operation.prepare()?;
        match issue.execute(|| {
            // SAFETY: fixed operation owner survives actual checked retirement after acceptance.
            let result = unsafe { ConnectNamedPipe(pipe, Some(operation.pointer())) };
            operation.accept(result, true).map_err(EntryTransportError::from)
        })? {
            IssueDispatch::Issued(accepted) => {
                let observed = operation.initial_progress(accepted, pipe)?;
                if matches!(observed, Progress::Complete(_)) {
                    slot.connected = true;
                }
                Ok(IssueDispatch::Issued(observed))
            }
            IssueDispatch::Closed => Ok(IssueDispatch::Closed),
            IssueDispatch::Expired => Ok(IssueDispatch::Expired),
        }
    }
    fn read(
        &mut self,
        index: usize,
        bytes: &mut [u8],
        issue: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        let slot = self.slot(index)?;
        let pipe = slot.pipe.raw();
        let operation = slot.operation(Role::Read)?;
        operation.prepare()?;
        match issue.execute(|| {
            // SAFETY: worker adopts fixed buffer before issue and retains it through actual settlement.
            let result = unsafe { ReadFile(pipe, Some(bytes), None, Some(operation.pointer())) };
            operation.accept(result, false).map_err(EntryTransportError::from)
        })? {
            IssueDispatch::Issued(accepted) => {
                let observed = operation.initial_progress(accepted, pipe)?;
                if matches!(observed, Progress::Complete(_)) {
                    slot.read_completed = true;
                }
                Ok(IssueDispatch::Issued(observed))
            }
            IssueDispatch::Closed => Ok(IssueDispatch::Closed),
            IssueDispatch::Expired => Ok(IssueDispatch::Expired),
        }
    }
    fn write(
        &mut self,
        index: usize,
        bytes: &[u8],
        issue: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        let slot = self.slot(index)?;
        let pipe = slot.pipe.raw();
        let operation = slot.operation(Role::Write)?;
        operation.prepare()?;
        match issue.execute(|| {
            // SAFETY: exact immutable buffer and prepared OVERLAPPED remain owned through settlement.
            let result = unsafe { WriteFile(pipe, Some(bytes), None, Some(operation.pointer())) };
            operation.accept(result, false).map_err(EntryTransportError::from)
        })? {
            IssueDispatch::Issued(accepted) => {
                Ok(IssueDispatch::Issued(operation.initial_progress(accepted, pipe)?))
            }
            IssueDispatch::Closed => Ok(IssueDispatch::Closed),
            IssueDispatch::Expired => Ok(IssueDispatch::Expired),
        }
    }
    fn poll(&mut self, index: usize, role: Role) -> Result<Progress, EntryTransportError> {
        let slot = self.slot(index)?;
        let pipe = slot.pipe.raw();
        let observed = slot.operation(role)?.poll(pipe)?;
        if matches!(observed, Progress::Complete(_)) {
            if role == Role::Read {
                slot.read_completed = true;
            }
            if role == Role::Handshake {
                slot.connected = true;
            }
        }
        Ok(observed)
    }
    fn quiesce(&mut self, index: usize, role: Role) -> Result<(), EntryTransportError> {
        self.slot(index)?.operation(role)?.quiesce().map_err(Into::into)
    }
    fn verify(&mut self, index: usize, message: bool) -> Result<KernelPeer, EntryTransportError> {
        let expected = self.expected;
        let own = self.identity.ok_or(EntryTransportError::InvalidPhase)?;
        let server = self.server;
        let slot = self.slot(index)?;
        if !slot.connected || (message && !slot.read_completed) {
            return Err(EntryTransportError::InvalidPhase);
        }
        slot.peer.verify(slot.pipe.raw(), server, &expected, own, message)
    }
    fn nonce(&mut self) -> Result<[u8; 32], EntryTransportError> {
        let mut nonce = [0; 32];
        // SAFETY: the system CSPRNG fills exactly this admitted fixed output.
        unsafe { BCryptGenRandom(None, &mut nonce, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
            .ok()
            .map_err(|cause| error("entry_nonce", cause))?;
        if nonce.iter().all(|byte| *byte == 0) {
            return Err(EntryTransportError::Authentication);
        }
        Ok(nonce)
    }
    fn retire_connection(
        &mut self,
        index: usize,
        reuse: bool,
    ) -> Result<bool, EntryTransportError> {
        let server = self.server;
        self.slot(index)?.retire(reuse, server)
    }
    fn retire(&mut self) -> Result<bool, EntryTransportError> {
        let mut settled = true;
        let mut first = None;
        for slot in &mut self.slots {
            match slot.retire(false, self.server) {
                Ok(retired) => settled &= retired,
                Err(error) => {
                    settled = false;
                    security::retain(&mut first, error);
                }
            }
        }
        if settled {
            for handle in [&mut self.own_token, &mut self.own_image] {
                if let Err(error) = handle.close("entry_close_own_security") {
                    settled = false;
                    security::retain(&mut first, error.into());
                }
            }
        }
        if let Some(error) = first {
            return Err(error);
        }
        Ok(settled)
    }
}
fn endpoint(namespace: sugarsubstitute_contracts::lifecycle::NamespaceId) -> [u16; 100] {
    let mut name = [0; 100];
    let prefix = b"\\\\.\\pipe\\SugarSubstitute.entry.v1.";
    for (index, byte) in prefix.iter().enumerate() {
        name[index] = u16::from(*byte);
    }
    let hex = b"0123456789abcdef";
    for (index, byte) in namespace.bytes().iter().enumerate() {
        name[prefix.len() + 2 * index] = u16::from(hex[usize::from(byte >> 4)]);
        name[prefix.len() + 2 * index + 1] = u16::from(hex[usize::from(byte & 15)]);
    }
    name
}
pub(super) fn error(
    stage: &'static str,
    cause: impl Error + Send + Sync + 'static,
) -> EntryTransportError {
    EntryTransportError::native(NativeObservationError::new(stage, cause))
}
impl From<native_pipe::NativePipeError> for EntryTransportError {
    fn from(cause: native_pipe::NativePipeError) -> Self {
        Self::Native { cause: cause.cause, identity_unrestored: cause.identity_unrestored }
    }
}

fn open_client(slot: &mut Slot, name: &[u16; 100]) -> Result<(), EntryTransportError> {
    let access = FILE_READ_DATA.0
        | FILE_WRITE_DATA.0
        | FILE_READ_ATTRIBUTES.0
        | FILE_WRITE_ATTRIBUTES.0
        | SYNCHRONIZE.0;
    // SAFETY: derived local name, restricted access, identification-only SQOS and overlapped mode.
    let raw = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            access,
            FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
            None,
        )
    }
    .map_err(|cause| error("entry_open_client", cause))?;
    adopt(&mut slot.pipe, raw, "entry_open_client")?;
    let mode = PIPE_READMODE_MESSAGE;
    // SAFETY: the retained owned handle grants attribute-write access for message-mode selection.
    unsafe { SetNamedPipeHandleState(slot.pipe.raw(), Some(&mode), None, None) }
        .map_err(|cause| error("entry_client_message_mode", cause))?;
    Ok(())
}
