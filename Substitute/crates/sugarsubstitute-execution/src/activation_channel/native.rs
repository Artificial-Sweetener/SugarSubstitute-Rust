// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create and validate exact connected asynchronous pipe endpoints before issuing any I/O.
use super::*;
use std::mem::size_of;
use windows::{
    Wdk::Storage::FileSystem::{
        FILE_MODE_INFORMATION, FILE_SYNCHRONOUS_IO_ALERT, FILE_SYNCHRONOUS_IO_NONALERT,
        FileModeInformation, NtQueryInformationFile,
    },
    Win32::{
        Foundation::{
            GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAG_PROTECT_FROM_CLOSE,
            SetHandleInformation, WIN32_ERROR,
        },
        Security::{
            Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom},
            SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ,
            FILE_GENERIC_WRITE, FILE_SHARE_MODE, FILE_TYPE_PIPE, GetFileType, OPEN_EXISTING,
            PIPE_ACCESS_DUPLEX,
        },
        System::{IO::IO_STATUS_BLOCK, Pipes::*, Threading::GetCurrentProcessId},
    },
    core::PCWSTR,
};

/// Compare Win32 statuses without losing the original native failure in diagnostics.
pub(super) fn code(error: &windows::core::Error, code: WIN32_ERROR) -> bool {
    error.code() == code.to_hresult()
}

/// Parent opens the sole client itself and verifies both endpoint identities before inheritance.
pub(super) fn pair(
    memory: Arc<MemoryClaim>,
) -> Result<(Handle, Handle, [u8; 32]), NativeObservationError> {
    let mut identity = [0; 32];
    // SAFETY: system RNG writes exactly the bounded nonce used for one private connection.
    unsafe { BCryptGenRandom(None, &mut identity, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok()
        .map_err(|cause| NativeObservationError::new("activation_channel_identity", cause))?;
    if identity == [0; 32] {
        return Err(invalid("activation_channel_identity"));
    }
    let suffix: String = identity.iter().map(|byte| format!("{byte:02x}")).collect();
    let name: Vec<u16> = format!(r"\\.\pipe\SugarSubstitute-activation-{suffix}")
        .encode_utf16()
        .chain([0])
        .collect();
    // SAFETY: unique local one-instance byte pipe; no endpoint can perform synchronous I/O.
    let server = unsafe {
        CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            65536,
            65536,
            0,
            None,
        )
    };
    if server.is_invalid() {
        return Err(NativeObservationError::new(
            "activation_channel_server",
            windows::core::Error::from_thread(),
        ));
    }
    let server = Handle::new(server, memory.clone());
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        bInheritHandle: true.into(),
        lpSecurityDescriptor: std::ptr::null_mut(),
    };
    // SAFETY: parent opens the exact private client; only this connected endpoint will inherit.
    let client = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            FILE_SHARE_MODE(0),
            Some(&attributes),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        )
    }
    .map_err(|cause| NativeObservationError::new("activation_channel_client", cause))?;
    let client = Handle::new(client, memory);
    let mut client_pid = 0;
    let mut server_pid = 0;
    // SAFETY: both endpoints remain owned; outputs are local initialized integers.
    unsafe {
        GetNamedPipeClientProcessId(server.raw(), &mut client_pid)
            .and_then(|()| GetNamedPipeServerProcessId(client.raw(), &mut server_pid))
    }
    .map_err(|cause| NativeObservationError::new("activation_channel_peer", cause))?;
    // SAFETY: current process identity is a kernel value, not a PID supplied by a peer.
    let current = unsafe { GetCurrentProcessId() };
    if client_pid != current || server_pid != current {
        return Err(invalid("activation_channel_peer"));
    }
    validate_async(client.raw())?;
    Ok((server, client, identity))
}

/// Verify the inherited client before any read/write can block a worker indefinitely.
pub(super) fn capture(raw: usize) -> Result<(), NativeObservationError> {
    if raw == 0 || raw > isize::MAX as usize {
        return Err(invalid("activation_channel_inherited_handle"));
    }
    let handle = HANDLE(raw as *mut _);
    let mut flags = 0;
    // SAFETY: initial entry guarantees that this unowned slot cannot be closed or reused concurrently.
    unsafe { GetHandleInformation(handle, &mut flags) }.map_err(|cause| {
        NativeObservationError::new("activation_channel_inherited_flags", cause)
    })?;
    if flags & HANDLE_FLAG_INHERIT.0 == 0 || flags & HANDLE_FLAG_PROTECT_FROM_CLOSE.0 != 0 {
        return Err(invalid("activation_channel_inherited_flags"));
    }
    validate_async(handle)?;
    let mut pipe_flags = NAMED_PIPE_MODE::default();
    // SAFETY: validated live pipe slot remains exclusively unowned through initial capture.
    unsafe { GetNamedPipeInfo(handle, Some(&mut pipe_flags), None, None, None) }
        .map_err(|cause| NativeObservationError::new("activation_channel_inherited_kind", cause))?;
    if pipe_flags.0 & (PIPE_SERVER_END.0 | PIPE_TYPE_MESSAGE.0) != 0 {
        return Err(invalid("activation_channel_inherited_kind"));
    }
    // SAFETY: narrowing inheritance prevents this private endpoint reaching application descendants.
    unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, Default::default()) }.map_err(
        |cause| NativeObservationError::new("activation_channel_narrow_inheritance", cause),
    )
}

/// Require pipe object type and asynchronous file mode before accepting peer-facing operations.
fn validate_async(handle: HANDLE) -> Result<(), NativeObservationError> {
    // SAFETY: callers retain an owned or exclusively unowned initial-entry slot throughout validation.
    if unsafe { GetFileType(handle) } != FILE_TYPE_PIPE {
        return Err(invalid("activation_channel_not_pipe"));
    }
    let mut status = IO_STATUS_BLOCK::default();
    let mut mode = FILE_MODE_INFORMATION::default();
    // SAFETY: FileModeInformation is a fixed synchronous metadata query with correctly sized output.
    let queried = unsafe {
        NtQueryInformationFile(
            handle,
            &mut status,
            std::ptr::from_mut(&mut mode).cast(),
            size_of::<FILE_MODE_INFORMATION>() as u32,
            FileModeInformation,
        )
    };
    queried.ok().map_err(|cause| NativeObservationError::new("activation_channel_mode", cause))?;
    if mode.Mode & (FILE_SYNCHRONOUS_IO_ALERT | FILE_SYNCHRONOUS_IO_NONALERT).0 != 0 {
        return Err(invalid("activation_channel_synchronous_handle"));
    }
    Ok(())
}
