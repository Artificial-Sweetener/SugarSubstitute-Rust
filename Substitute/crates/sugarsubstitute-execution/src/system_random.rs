// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fill caller-owned bytes from the operating system's random provider.
//!
//! Used for unguessable private channel identities and WebSocket keys and masks.
#[cfg(not(windows))]
use std::io::Read;

/// Fill `bytes` completely from the system provider.
/// # Errors
/// Returns the provider's failure; `bytes` is then unspecified and must not be used.
#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) fn fill(bytes: &mut [u8]) -> std::io::Result<()> {
    use windows::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    // SAFETY: The mutable slice is live for this synchronous call; the system
    // provider takes no owned handle and retains no pointer after return.
    unsafe { BCryptGenRandom(None, bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok()
        .map_err(std::io::Error::other)
}

/// Fill `bytes` completely from the fixed OS entropy device.
/// # Errors
/// Returns the device's open or read failure.
#[cfg(not(windows))]
pub(crate) fn fill(bytes: &mut [u8]) -> std::io::Result<()> {
    std::fs::File::open("/dev/urandom")?.read_exact(bytes)
}
