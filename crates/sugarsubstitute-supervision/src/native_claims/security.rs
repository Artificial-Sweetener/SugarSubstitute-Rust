// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare bounded native security without allocating foreign descriptor strings.
mod descriptor;
use super::{ClaimError, files::Handle};
use windows::{Win32::Security::*, core::PWSTR};
pub(crate) struct Descriptor {
    words: Vec<u32>,
}
impl Descriptor {
    pub(crate) fn new(sid: &[u8]) -> Result<Self, ClaimError> {
        let bytes = descriptor::expected(sid)?;
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
pub(crate) fn verify(handle: &Handle, sid: &[u8]) -> Result<(), ClaimError> {
    let mut words = vec![0_u32; descriptor::MAXIMUM_BYTES / 4];
    let mut needed = 0;
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
    .map_err(|e| ClaimError::new("query_security", e))?;
    if !(20..=descriptor::MAXIMUM_BYTES).contains(&(needed as usize)) {
        return Err(ClaimError::invalid("security_descriptor_bound"));
    }
    // SAFETY: Successful native output reported this checked byte count inside the owned allocation.
    let bytes = unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), needed as usize) };
    descriptor::verify(bytes, sid)
}
pub(crate) unsafe fn bounded_text(text: PWSTR, maximum: usize) -> Result<String, ClaimError> {
    if text.is_null() {
        return Err(ClaimError::invalid("native_text"));
    }
    let mut units = Vec::new();
    for index in 0..maximum {
        // SAFETY: The caller supplies a valid NUL-terminated allocation; scanning stops at its terminator.
        let unit = unsafe { text.0.add(index).read() };
        if unit == 0 {
            return String::from_utf16(&units).map_err(|e| ClaimError::new("native_unicode", e));
        }
        units.push(unit);
    }
    Err(ClaimError::invalid("native_text_bound"))
}
