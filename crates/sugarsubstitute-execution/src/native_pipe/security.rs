// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify bounded account/logon/session token facts and explicit local pipe access.
use super::{NativePipeError, handles::*};
use windows::Win32::Security::*;
use windows::Win32::System::SystemServices::{SE_GROUP_LOGON_ID, SECURITY_DESCRIPTOR_REVISION};
use windows::Win32::{
    Foundation::*,
    Storage::FileSystem::*,
    System::{Pipes::ImpersonateNamedPipeClient, Threading::*},
};
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct Sid {
    bytes: [u8; 68],
    length: usize,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct Identity {
    account: Sid,
    logon: Sid,
    pub(crate) session: u32,
}
impl Identity {
    /// Borrow exact kernel-validated account SID bytes without granting native authority.
    pub(crate) fn account_sid(&self) -> &[u8] {
        &self.account.bytes[..self.account.length]
    }
}
#[cfg(test)]
impl Identity {
    pub(crate) fn fixture(account: u8, logon: u8, session: u32) -> Self {
        Self {
            account: Sid { bytes: [account; 68], length: 68 },
            logon: Sid { bytes: [logon; 68], length: 68 },
            session,
        }
    }
}
fn sid(buffer: &[usize; 256], returned: usize, sid: PSID) -> Result<Sid, NativePipeError> {
    let start = buffer.as_ptr() as usize;
    let end = start.checked_add(returned).ok_or_else(|| invariant("pipe_sid_bound"))?;
    let pointer = sid.0 as usize;
    if pointer < start || pointer.checked_add(8).is_none_or(|p| p > end) {
        return Err(invariant("pipe_sid_bound"));
    }
    // SAFETY: the complete fixed SID header lies inside returned token output.
    let count = unsafe { sid.0.cast::<u8>().add(1).read() } as usize;
    let length = 8 + 4 * count;
    if count > 15 || length > 68 || pointer.checked_add(length).is_none_or(|p| p > end) {
        return Err(invariant("pipe_sid_bound"));
    }
    // SAFETY: all declared SID subauthorities were bounded inside the token allocation.
    if !unsafe { IsValidSid(sid) }.as_bool() || unsafe { GetLengthSid(sid) } as usize != length {
        return Err(invariant("pipe_invalid_sid"));
    }
    let mut bytes = [0; 68];
    // SAFETY: this exact validated range remains within the live token allocation.
    bytes[..length]
        .copy_from_slice(unsafe { std::slice::from_raw_parts(sid.0.cast::<u8>(), length) });
    Ok(Sid { bytes, length })
}
fn information(
    token: HANDLE,
    class: TOKEN_INFORMATION_CLASS,
    buffer: &mut [usize; 256],
) -> Result<usize, NativePipeError> {
    let mut returned = 0;
    // SAFETY: aligned initialized fixed output is advertised at its exact byte size.
    unsafe {
        GetTokenInformation(
            token,
            class,
            Some(buffer.as_mut_ptr().cast()),
            size_of_val(buffer) as u32,
            &mut returned,
        )
    }
    .map_err(|e| error("pipe_token_information", e))?;
    if returned as usize > size_of_val(buffer) {
        return Err(invariant("pipe_token_information_bound"));
    }
    Ok(returned as usize)
}
pub(crate) fn identity(token: HANDLE) -> Result<Identity, NativePipeError> {
    let mut buffer = [0usize; 256];
    let size = information(token, TokenUser, &mut buffer)?;
    if size < size_of::<TOKEN_USER>() {
        return Err(invariant("pipe_token_user_header"));
    }
    // SAFETY: successful bounded output contains the aligned header.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let account = sid(&buffer, size, user.User.Sid)?;
    let mut groups = [0usize; 256];
    let size = information(token, TokenLogonSid, &mut groups)?;
    if size < size_of::<TOKEN_GROUPS>() {
        return Err(invariant("pipe_token_logon_header"));
    }
    // SAFETY: the fixed one-group header lies inside successful token output.
    let group = unsafe { &*groups.as_ptr().cast::<TOKEN_GROUPS>() };
    if group.GroupCount != 1
        || group.Groups[0].Attributes & SE_GROUP_LOGON_ID as u32 != SE_GROUP_LOGON_ID as u32
    {
        return Err(invariant("pipe_token_logon_group"));
    }
    let logon = sid(&groups, size, group.Groups[0].Sid)?;
    let mut session = [0usize; 256];
    let size = information(token, TokenSessionId, &mut session)?;
    if size != size_of::<u32>() {
        return Err(invariant("pipe_token_session_bound"));
    }
    // SAFETY: exact successful DWORD output is aligned and initialized.
    let session = unsafe { session.as_ptr().cast::<u32>().read() };
    Ok(Identity { account, logon, session })
}
pub(crate) fn process_identity(
    process: HANDLE,
    token: &mut Handle,
) -> Result<Identity, NativePipeError> {
    let mut raw = HANDLE::default();
    // SAFETY: process is retained by the caller, and raw output is adopted before checking.
    let result = unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw) };
    if !raw.is_invalid() {
        token.0 = raw.0 as usize;
    }
    result.map_err(|e| error("pipe_open_process_token", e))?;
    if token.0 == 0 {
        return Err(invariant("pipe_missing_process_token"));
    }
    identity(token.raw())
}
pub(crate) fn process_facts(process: HANDLE) -> Result<(u32, u64), NativePipeError> {
    // SAFETY: caller retains a process handle with QUERY_LIMITED_INFORMATION rights.
    let pid = unsafe { GetProcessId(process) };
    if pid == 0 {
        return Err(error("pipe_process_id", last_error()));
    }
    let mut birth = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: retained handle and live fixed FILETIME outputs remain valid for this finite query.
    unsafe { GetProcessTimes(process, &mut birth, &mut exit, &mut kernel, &mut user) }
        .map_err(|e| error("pipe_process_birth", e))?;
    let birth = ((birth.dwHighDateTime as u64) << 32) | birth.dwLowDateTime as u64;
    if birth == 0 {
        return Err(invariant("pipe_positive_process_birth"));
    }
    Ok((pid, birth))
}
pub(crate) struct Descriptor {
    descriptor: SECURITY_DESCRIPTOR,
    acl: Box<[u32; 128]>,
}
impl Descriptor {
    pub(crate) fn new(identity: Identity) -> Result<Self, NativePipeError> {
        let mut value =
            Self { descriptor: SECURITY_DESCRIPTOR::default(), acl: Box::new([0; 128]) };
        // SAFETY: descriptor and aligned ACL storage are fully owned through native object creation.
        unsafe {
            InitializeSecurityDescriptor(
                PSECURITY_DESCRIPTOR((&mut value.descriptor as *mut SECURITY_DESCRIPTOR).cast()),
                SECURITY_DESCRIPTOR_REVISION,
            )?;
            InitializeAcl(
                value.acl.as_mut_ptr().cast(),
                size_of_val(value.acl.as_ref()) as u32,
                ACL_REVISION,
            )?;
            AddAccessAllowedAceEx(
                value.acl.as_mut_ptr().cast(),
                ACL_REVISION,
                ACE_FLAGS(0),
                FILE_READ_DATA.0
                    | FILE_WRITE_DATA.0
                    | FILE_READ_ATTRIBUTES.0
                    | FILE_WRITE_ATTRIBUTES.0
                    | SYNCHRONIZE.0,
                PSID(identity.logon.bytes.as_ptr() as *mut _),
            )?;
            SetSecurityDescriptorDacl(
                PSECURITY_DESCRIPTOR((&mut value.descriptor as *mut SECURITY_DESCRIPTOR).cast()),
                true,
                Some(value.acl.as_ptr().cast()),
                false,
            )?;
            SetSecurityDescriptorControl(
                PSECURITY_DESCRIPTOR((&mut value.descriptor as *mut SECURITY_DESCRIPTOR).cast()),
                SE_DACL_PROTECTED,
                SE_DACL_PROTECTED,
            )?;
        }
        Ok(value)
    }
    pub(crate) fn attributes(&mut self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: (&mut self.descriptor as *mut SECURITY_DESCRIPTOR).cast(),
            bInheritHandle: false.into(),
        }
    }
}
impl From<windows::core::Error> for NativePipeError {
    fn from(value: windows::core::Error) -> Self {
        error("pipe_security_descriptor", value)
    }
}
pub(crate) fn message_identity(
    pipe: HANDLE,
    token: &mut Handle,
) -> Result<Identity, NativePipeError> {
    // SAFETY: pipe belongs to this native worker and a bounded message was actually read.
    unsafe { ImpersonateNamedPipeClient(pipe) }.map_err(|e| error("pipe_impersonate", e))?;
    // Restore identity even if a private Rust error allocation/panic occurs while inspecting it.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut raw = HANDLE::default();
        // SAFETY: pseudo-thread is current; output is durably adopted before checking.
        let opened = unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, true, &mut raw) };
        if !raw.is_invalid() {
            token.0 = raw.0 as usize;
        }
        opened.map_err(|e| error("pipe_open_message_token", e))?;
        let mut level = [0usize; 256];
        let size = information(token.raw(), TokenImpersonationLevel, &mut level)?;
        if size != size_of::<SECURITY_IMPERSONATION_LEVEL>() {
            return Err(invariant("pipe_impersonation_level_bound"));
        }
        // SAFETY: exact aligned enum-sized output was returned by Windows.
        let level = unsafe { level.as_ptr().cast::<SECURITY_IMPERSONATION_LEVEL>().read() };
        if level.0 < SecurityIdentification.0 {
            return Err(invariant("pipe_insufficient_identification"));
        }
        identity(token.raw())
    }));
    // SAFETY: restores this exact thread; failure is fatal, not retryable authentication.
    let reverted = unsafe { RevertToSelf() }.map_err(|cause| error("pipe_revert_identity", cause));
    restored(result, reverted)
}
fn restored(
    result: Result<Result<Identity, NativePipeError>, Box<dyn std::any::Any + Send>>,
    reverted: Result<(), NativePipeError>,
) -> Result<Identity, NativePipeError> {
    match (result, reverted) {
        (Ok(result), Ok(())) => result,
        (Err(payload), Ok(())) => std::panic::resume_unwind(payload),
        (Ok(Err(mut first)), Err(cause)) => {
            first.identity_unrestored = true;
            first.cause.retain_cleanup(cause.cause);
            Err(first)
        }
        (Ok(Ok(_)), Err(cause)) => {
            let mut error = cause;
            error.identity_unrestored = true;
            Err(error)
        }
        (Err(payload), Err(cause)) => {
            let mut error = cause;
            error.identity_unrestored = true;
            error.cause.retain_panic(&crate::task::panic_cause(payload));
            Err(error)
        }
    }
}

fn invariant(stage: &'static str) -> NativePipeError {
    error(stage, std::io::Error::other("Native pipe credential invariant rejected"))
}

#[cfg(test)]
#[path = "security_tests.rs"]
mod tests;
