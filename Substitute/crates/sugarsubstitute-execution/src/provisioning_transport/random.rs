// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fill a caller-owned connection capability with the Windows system random provider.
use windows::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};

pub(super) fn fill(bytes: &mut [u8; 32]) -> std::io::Result<()> {
    // SAFETY: The fixed mutable slice is live for this synchronous call; the system
    // provider takes no owned handle and retains no pointer after return.
    unsafe { BCryptGenRandom(None, bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok()
        .map_err(std::io::Error::other)
}
