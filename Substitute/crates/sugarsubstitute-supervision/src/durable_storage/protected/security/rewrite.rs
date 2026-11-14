// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Rewrite a real file's DACL the way permission tools and restores do, for qualification.
use super::descriptor::{SYSTEM, ace, acl, owner};
use std::{io, path::Path};
use windows::{
    Win32::Security::{
        DACL_SECURITY_INFORMATION, GetFileSecurityW, OWNER_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SetFileSecurityW,
        UNPROTECTED_DACL_SECURITY_INFORMATION,
    },
    core::HSTRING,
};

/// The DACL a tool leaves behind.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Rewrite {
    /// An icacls round trip and exact restore: SYSTEM first, no inheritance flags.
    SameAccessReordered,
    /// The same, after denying the owner execute access.
    DenyExecute,
    /// A tool that also lets every user of the machine change the file.
    UsersMayWrite,
    /// A tool that keeps the same grants but lets the file inherit its folder's again.
    InheritsAgain,
}

const FULL: u32 = 0x001f01ff;
const EXECUTE: u32 = 0x20;
const USERS: &[u8] = &[1, 2, 0, 0, 0, 0, 0, 5, 32, 0, 0, 0, 33, 2, 0, 0];

/// Replace the protected DACL of `path` as `rewrite` describes, keeping its owner and label.
pub(crate) fn rewrite(path: &Path, rewrite: Rewrite) -> io::Result<()> {
    let name = HSTRING::from(path.as_os_str());
    let mut words = vec![0_u32; 2048];
    let mut needed = 0;
    // SAFETY: The DWORD-aligned buffer holds 8192 writable bytes for this synchronous query.
    unsafe {
        GetFileSecurityW(
            &name,
            OWNER_SECURITY_INFORMATION.0,
            Some(PSECURITY_DESCRIPTOR(words.as_mut_ptr().cast())),
            8192,
            &mut needed,
        )
    }
    .ok()
    .map_err(io::Error::other)?;
    // SAFETY: The query reported `needed` initialized bytes inside the owned buffer.
    let current =
        unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), needed as usize) };
    let account = owner(current).map_err(io::Error::other)?.to_vec();
    let aces = match rewrite {
        Rewrite::SameAccessReordered => vec![ace(0, 0, FULL, SYSTEM), ace(0, 0, FULL, &account)],
        Rewrite::DenyExecute => {
            vec![ace(1, 0, EXECUTE, &account), ace(0, 0, FULL, SYSTEM), ace(0, 0, FULL, &account)]
        }
        Rewrite::UsersMayWrite => {
            vec![ace(0, 0, FULL, SYSTEM), ace(0, 0, FULL, &account), ace(0, 0, 0x0012019f, USERS)]
        }
        Rewrite::InheritsAgain => vec![ace(0, 0, FULL, &account), ace(0, 0, FULL, SYSTEM)],
    };
    let (control, inheritance) = match rewrite {
        Rewrite::InheritsAgain => (0x8000_u16 | 0x0004, UNPROTECTED_DACL_SECURITY_INFORMATION),
        _ => (0x8000_u16 | 0x1000 | 0x0004, PROTECTED_DACL_SECURITY_INFORMATION),
    };
    let dacl = acl(&aces);
    // Self-relative with a DACL, protected unless it inherits; the DACL follows the header.
    let mut bytes = vec![0_u8; 20];
    bytes[0] = 1;
    bytes[2..4].copy_from_slice(&control.to_le_bytes());
    bytes[16..20].copy_from_slice(&20_u32.to_le_bytes());
    bytes.extend_from_slice(&dacl);
    let mut words = vec![0_u32; bytes.len().div_ceil(4)];
    // SAFETY: Both buffers are live and the destination holds at least `bytes.len()` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), words.as_mut_ptr().cast(), bytes.len())
    };
    // SAFETY: The self-relative descriptor stays alive and aligned for this synchronous call.
    unsafe {
        SetFileSecurityW(
            &name,
            DACL_SECURITY_INFORMATION | inheritance,
            PSECURITY_DESCRIPTOR(words.as_mut_ptr().cast()),
        )
    }
    .ok()
    .map_err(io::Error::other)
}
