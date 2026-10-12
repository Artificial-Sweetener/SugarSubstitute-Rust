// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin actual peer processes, primary/message tokens and installed executable identities.
use super::native::error;
use crate::{
    coordinator_entry::{EntryTransportError, KernelPeer},
    native_pipe::{
        handles::{Handle, adopt},
        security::{self, Identity},
    },
};
use sugarsubstitute_contracts::coordinator_entry::EntryNativeIdentityFacts;
// GetDriveTypeW's DRIVE_FIXED value is defined by the Windows SDK independently of file flags.
const DRIVE_FIXED: u32 = 3;
use windows::{
    Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{Pipes::*, Threading::*},
    },
    core::{PCWSTR, PWSTR},
};

pub(super) struct PeerPin {
    process: Handle,
    token: Handle,
    image: Handle,
    message_token: Handle,
    pid: u32,
    birth: u64,
    image_identity: Option<[u8; 24]>,
}
impl PeerPin {
    pub(super) fn new() -> Self {
        Self {
            process: Handle(0),
            token: Handle(0),
            image: Handle(0),
            message_token: Handle(0),
            pid: 0,
            birth: 0,
            image_identity: None,
        }
    }
    pub(super) fn verify(
        &mut self,
        pipe: HANDLE,
        server: bool,
        expected: &EntryNativeIdentityFacts,
        own: Identity,
        message: bool,
    ) -> Result<KernelPeer, EntryTransportError> {
        let mut pid = 0;
        let mut session = 0;
        // SAFETY: the retained connected pipe and fixed DWORD outputs remain live.
        unsafe {
            if server {
                GetNamedPipeClientProcessId(pipe, &mut pid)?;
                GetNamedPipeClientSessionId(pipe, &mut session)?;
            } else {
                GetNamedPipeServerProcessId(pipe, &mut pid)?;
                GetNamedPipeServerSessionId(pipe, &mut session)?;
            }
        }
        if pid == 0 || session != expected.session_id() || (self.pid != 0 && pid != self.pid) {
            return Err(EntryTransportError::Authentication);
        }
        if self.process.0 == 0 {
            // SAFETY: only query and synchronization rights are requested for the actual pipe peer.
            let process = unsafe {
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE, false, pid)
            }
            .map_err(|cause| error("entry_open_actual_peer", cause))?;
            adopt(&mut self.process, process, "entry_open_actual_peer")?;
        }
        alive(self.process.raw())?;
        let before = security::process_facts(self.process.raw())?;
        if before.0 != pid || (self.birth != 0 && before.1 != self.birth) {
            return Err(EntryTransportError::Authentication);
        }
        self.pid = before.0;
        self.birth = before.1;
        self.token.close("entry_close_previous_primary_token")?;
        let identity = security::process_identity(self.process.raw(), &mut self.token)?;
        if identity != own {
            return Err(EntryTransportError::Authentication);
        }
        if self.image_identity.is_none() {
            self.image_identity = Some(image_identity(self.process.raw(), &mut self.image)?);
        }
        let image = self.image_identity.ok_or(EntryTransportError::Authentication)?;
        if image != expected.host_image_identity() {
            return Err(EntryTransportError::Authentication);
        }
        if message && server {
            self.message_token.close("entry_close_previous_message_token")?;
            let actual = security::message_identity(pipe, &mut self.message_token)?;
            if actual != own {
                return Err(EntryTransportError::Authentication);
            }
        }
        let after = security::process_facts(self.process.raw())?;
        alive(self.process.raw())?;
        if before != after {
            return Err(EntryTransportError::Authentication);
        }
        let facts = EntryNativeIdentityFacts::new(
            expected.namespace(),
            identity.account_sid(),
            identity.session,
            image,
        )
        .map_err(|_| EntryTransportError::Authentication)?;
        Ok(KernelPeer { facts, pid, birth: before.1 })
    }
    pub(super) fn retire(&mut self) -> Result<(), EntryTransportError> {
        let mut first: Option<EntryTransportError> = None;
        for handle in [&mut self.message_token, &mut self.token, &mut self.image, &mut self.process]
        {
            if let Err(error) = handle.close("entry_close_peer_security") {
                retain(&mut first, error.into());
            }
        }
        if let Some(error) = first {
            return Err(error);
        }
        self.pid = 0;
        self.birth = 0;
        self.image_identity = None;
        Ok(())
    }
}

pub(super) fn own_identity(
    expected: &EntryNativeIdentityFacts,
    token: &mut Handle,
    image: &mut Handle,
) -> Result<Identity, EntryTransportError> {
    // SAFETY: the current-process pseudo handle is retained by Windows and never closed here.
    let process = unsafe { GetCurrentProcess() };
    let identity = security::process_identity(process, token)?;
    let actual_image = image_identity(process, image)?;
    if identity.account_sid() != expected.account_sid()
        || identity.session != expected.session_id()
        || actual_image != expected.host_image_identity()
    {
        return Err(EntryTransportError::Authentication);
    }
    Ok(identity)
}
fn alive(process: HANDLE) -> Result<(), EntryTransportError> {
    // SAFETY: the private process slot owns synchronization rights; zero timeout never waits.
    match unsafe { WaitForSingleObject(process, 0) } {
        WAIT_TIMEOUT => Ok(()),
        WAIT_OBJECT_0 => Err(EntryTransportError::Authentication),
        _ => Err(error("entry_query_peer_liveness", std::io::Error::last_os_error())),
    }
}
fn image_identity(process: HANDLE, image: &mut Handle) -> Result<[u8; 24], EntryTransportError> {
    let mut path = [0u16; 4096];
    let mut length = path.len() as u32;
    // SAFETY: exact fixed output capacity is supplied and the process query handle remains live.
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(path.as_mut_ptr()),
            &mut length,
        )
    }
    .map_err(|cause| error("entry_query_peer_image", cause))?;
    let length = length as usize;
    if length < 3 || length >= path.len() || path[..length].contains(&0) {
        return Err(EntryTransportError::Authentication);
    }
    let drive = u8::try_from(path[0]).map_err(|_| EntryTransportError::Authentication)?;
    if !drive.is_ascii_alphabetic() || path[1] != u16::from(b':') || path[2] != u16::from(b'\\') {
        return Err(EntryTransportError::Authentication);
    }
    let root = [path[0], path[1], path[2], 0];
    // SAFETY: bounded literal drive root prevents image inspection from reaching a network drive.
    if unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } != DRIVE_FIXED {
        return Err(EntryTransportError::Authentication);
    }
    path[length] = 0;
    // SAFETY: actual kernel image path, finite query access and read-only sharing pin its file.
    let file = unsafe {
        CreateFileW(
            PCWSTR(path.as_ptr()),
            FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(|cause| error("entry_pin_peer_image", cause))?;
    adopt(image, file, "entry_pin_peer_image")?;
    let mut info = FILE_ID_INFO::default();
    // SAFETY: retained image handle and exact aligned fixed file-ID output remain live.
    unsafe {
        GetFileInformationByHandleEx(
            image.raw(),
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    }
    .map_err(|cause| error("entry_peer_image_identity", cause))?;
    let mut identity = [0; 24];
    identity[..8].copy_from_slice(&info.VolumeSerialNumber.to_le_bytes());
    identity[8..].copy_from_slice(&info.FileId.Identifier);
    Ok(identity)
}
pub(super) fn retain(first: &mut Option<EntryTransportError>, next: EntryTransportError) {
    match first {
        Some(first) => first.retain_cleanup(next),
        first => *first = Some(next),
    }
}
impl From<windows::core::Error> for EntryTransportError {
    fn from(cause: windows::core::Error) -> Self {
        error("entry_query_kernel_peer", cause)
    }
}
