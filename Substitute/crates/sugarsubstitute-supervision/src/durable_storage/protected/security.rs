// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare bounded native security without allocating foreign descriptor strings.
mod descriptor;
#[cfg(test)]
pub(crate) mod rewrite;
use super::native_phase::NativePhase;
use crate::claim_error::BoundedNativeCause;
use crate::{ClaimError, native_claims::files::Handle};
use windows::Win32::Security::*;
pub(crate) struct Descriptor {
    words: Vec<u32>,
}
impl Descriptor {
    pub(crate) fn new(sid: &[u8], phase: &mut NativePhase<'_>) -> Result<Self, ClaimError> {
        phase.security()?;
        let bytes = descriptor::expected(sid).map_err(|operation| phase.invalid(operation))?;
        Ok(Self::from_bytes(bytes))
    }
    /// Build the closed recovery Job security policy using the same checked SID serializer.
    pub(crate) fn recovery_job(
        sid: &[u8],
        phase: &mut NativePhase<'_>,
    ) -> Result<Self, ClaimError> {
        phase.security()?;
        let bytes =
            descriptor::expected_recovery_job(sid).map_err(|operation| phase.invalid(operation))?;
        Ok(Self::from_bytes(bytes))
    }
    fn from_bytes(bytes: Vec<u8>) -> Self {
        let words = bytes
            .chunks(4)
            .map(|part| {
                u32::from_le_bytes(std::array::from_fn(|i| part.get(i).copied().unwrap_or(0)))
            })
            .collect();
        Self { words }
    }
    pub(crate) fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.words.as_ptr() as *mut _,
            bInheritHandle: false.into(),
        }
    }
}
/// Whether `error` was caused by protected storage whose security another account, an
/// unprotected DACL, a foreign owner or a lowered label has made untrustworthy.
pub(crate) fn untrusted(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(cause) = current {
        if let Some(claim) = cause.downcast_ref::<ClaimError>()
            && descriptor::UNTRUSTED.contains(&claim.operation())
        {
            return true;
        }
        // An I/O error's `source` skips the error it wraps, so look inside it first.
        current = match cause.downcast_ref::<std::io::Error>().and_then(std::io::Error::get_ref) {
            Some(inner) => Some(inner),
            None => cause.source(),
        };
    }
    false
}

/// Protect the DACL of the object at `path` again when inheriting is all that is wrong with
/// its security: every grant still names this account (`sid`) or SYSTEM and the owner and label
/// are this account's. The DACL becomes the one this account gives its storage. Returns whether
/// the object was repaired; objects with any other security, and links, are left as they are.
/// # Errors
/// The object cannot be opened, read or changed, or it is still untrusted after the change.
pub(crate) fn reprotect(path: &std::path::Path, sid: &[u8]) -> std::io::Result<bool> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            READ_CONTROL, WRITE_DAC,
        },
    };
    let file = std::fs::OpenOptions::new()
        .access_mode((READ_CONTROL | WRITE_DAC).0)
        .share_mode(7)
        .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
        .open(path)?;
    use std::os::windows::fs::MetadataExt;
    if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Ok(false);
    }
    let handle = HANDLE(file.as_raw_handle());
    if descriptor::verify(&query(handle)?, sid) != Err(descriptor::UNPROTECTED) {
        return Ok(false);
    }
    let expected =
        Descriptor::from_bytes(descriptor::expected(sid).map_err(std::io::Error::other)?);
    // SAFETY: The handle stays open for this synchronous call and the self-relative descriptor
    // is DWORD-aligned and alive; only its DACL is applied, protected from inheritance.
    unsafe {
        SetKernelObjectSecurity(
            handle,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            PSECURITY_DESCRIPTOR(expected.words.as_ptr() as *mut _),
        )
    }
    .map_err(std::io::Error::other)?;
    descriptor::verify(&query(handle)?, sid).map_err(std::io::Error::other)?;
    Ok(true)
}

/// The owner, DACL and label of the object `handle` opens, as a self-relative descriptor.
fn query(handle: windows::Win32::Foundation::HANDLE) -> std::io::Result<Vec<u8>> {
    let mut words = vec![0_u32; descriptor::MAXIMUM_BYTES / 4];
    let mut needed = 0;
    // SAFETY: The DWORD-aligned buffer owns `MAXIMUM_BYTES` writable bytes; Windows copies a
    // descriptor only when it fits and reports its size.
    unsafe {
        GetKernelObjectSecurity(
            handle,
            INFORMATION,
            Some(PSECURITY_DESCRIPTOR(words.as_mut_ptr().cast())),
            descriptor::MAXIMUM_BYTES as u32,
            &mut needed,
        )
    }
    .map_err(std::io::Error::other)?;
    let length = (needed as usize).min(descriptor::MAXIMUM_BYTES);
    // SAFETY: Windows initialized `needed` bytes of the owned buffer.
    Ok(unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), length) }.to_vec())
}

const INFORMATION: u32 =
    OWNER_SECURITY_INFORMATION.0 | DACL_SECURITY_INFORMATION.0 | LABEL_SECURITY_INFORMATION.0;
pub(crate) fn verify(
    handle: &Handle,
    sid: &[u8],
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    verify_borrowed(handle.raw(), &handle.budget(), sid, phase)
}
/// Verify a caller-retained kernel object using the same bounded authoritative descriptor policy.
/// The native adapter retains unique handle ownership and supplies its original admitted budget.
pub(crate) fn verify_borrowed(
    handle: windows::Win32::Foundation::HANDLE,
    budget: &crate::native_claims::files::Budget,
    sid: &[u8],
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    verify_borrowed_using(handle, budget, sid, phase, descriptor::verify)
}
/// Verify the exact original recovery Job security policy while its native adapter retains it.
pub(crate) fn verify_recovery_job_borrowed(
    handle: windows::Win32::Foundation::HANDLE,
    budget: &crate::native_claims::files::Budget,
    sid: &[u8],
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    verify_borrowed_using(handle, budget, sid, phase, descriptor::verify_recovery_job)
}
fn verify_borrowed_using(
    handle: windows::Win32::Foundation::HANDLE,
    budget: &crate::native_claims::files::Budget,
    sid: &[u8],
    phase: &mut NativePhase<'_>,
    verify: fn(&[u8], &[u8]) -> Result<(), &'static str>,
) -> Result<(), ClaimError> {
    phase.check_budget(budget)?;
    phase.security()?;
    let mut words = vec![0_u32; descriptor::MAXIMUM_BYTES / 4];
    let mut needed = 0;
    phase.check()?;
    // SAFETY: This DWORD-aligned initialized buffer owns exactly8192 writable bytes.
    // Windows returns a self-relative descriptor, or reports insufficient capacity
    // without copying it. No native allocation depends on the foreign ACL size.
    unsafe {
        GetKernelObjectSecurity(
            handle,
            INFORMATION,
            Some(PSECURITY_DESCRIPTOR(words.as_mut_ptr().cast())),
            descriptor::MAXIMUM_BYTES as u32,
            &mut needed,
        )
    }
    .map_err(|cause| phase.error("query_security", BoundedNativeCause::Windows(cause)))?;
    if !(20..=descriptor::MAXIMUM_BYTES).contains(&(needed as usize)) {
        return Err(phase.invalid("security_descriptor_bound"));
    }
    // SAFETY: Successful native output reported this checked byte count inside the owned allocation.
    let bytes = unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), needed as usize) };
    verify(bytes, sid).map_err(|operation| phase.invalid(operation))?;
    phase.check()
}

/// Describe the authoritative fixed query and expected-descriptor overlap.
/// The descriptor owner accepts at most68 SID bytes and8192 query bytes.
pub(crate) fn diagnostic_security_layout() -> Result<
    crate::native_claims::files::NativeRoleLayout,
    crate::native_claims::files::NativeLayoutFailure,
> {
    use crate::{
        diagnostic_storage::{DiagnosticNativeScratchLoan, DiagnosticNativeSecurityPhase},
        native_claims::files::{NativeLayoutFailure, NativeRoleLayout},
    };
    const EXPECTED_BYTES: usize = 20 + 68 + (8 + 76 + 20) + (8 + 20);
    const EXPECTED_VECTOR_OVERLAP: usize = 3 * (76 + 20 + 20 + 104 + 28 + EXPECTED_BYTES);
    // Three ACEs, two ACLs, the expected result, query DWORDs and descriptor DWORDs.
    let controls = (8 * size_of::<Vec<u8>>())
        .checked_add(size_of::<DiagnosticNativeScratchLoan>())
        .and_then(|value| value.checked_add(size_of::<DiagnosticNativeSecurityPhase<'_>>()))
        .ok_or(NativeLayoutFailure::Overflow)?;
    let peak = (descriptor::MAXIMUM_BYTES + EXPECTED_VECTOR_OVERLAP)
        .checked_add(controls)
        .ok_or(NativeLayoutFailure::Overflow)?;
    Ok(NativeRoleLayout { retained: 0, peak })
}
