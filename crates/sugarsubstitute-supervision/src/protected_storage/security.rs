// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare bounded native security without allocating foreign descriptor strings.
mod descriptor;
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
        let words = bytes
            .chunks(4)
            .map(|part| {
                u32::from_le_bytes(std::array::from_fn(|i| part.get(i).copied().unwrap_or(0)))
            })
            .collect();
        Ok(Self { words })
    }
    pub(crate) fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.words.as_ptr() as *mut _,
            bInheritHandle: false.into(),
        }
    }
}
const INFORMATION: u32 =
    OWNER_SECURITY_INFORMATION.0 | DACL_SECURITY_INFORMATION.0 | LABEL_SECURITY_INFORMATION.0;
pub(crate) fn verify(
    handle: &Handle,
    sid: &[u8],
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    phase.check_budget(&handle.budget())?;
    phase.security()?;
    let mut words = vec![0_u32; descriptor::MAXIMUM_BYTES / 4];
    let mut needed = 0;
    phase.check()?;
    // SAFETY: This DWORD-aligned initialized buffer owns exactly8192 writable bytes.
    // Windows returns a self-relative descriptor, or reports insufficient capacity
    // without copying it. No native allocation depends on the foreign ACL size.
    unsafe {
        GetKernelObjectSecurity(
            handle.raw(),
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
    descriptor::verify(bytes, sid).map_err(|operation| phase.invalid(operation))?;
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
