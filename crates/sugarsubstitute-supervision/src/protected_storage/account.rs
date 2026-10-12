// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read the bounded SID from an actual retained primary token.
use crate::ClaimError;
use windows::Win32::{Foundation::HANDLE, Security::*};
pub(crate) fn token_sid(token: HANDLE) -> Result<Vec<u8>, ClaimError> {
    // TOKEN_USER and its referenced SID share this aligned, strictly bounded buffer.
    let mut buffer = vec![0_usize; 256];
    let mut returned = 0;
    // SAFETY: The aligned initialized buffer owns the advertised writable byte count.
    unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            (buffer.len() * size_of::<usize>()) as u32,
            &mut returned,
        )
    }
    .map_err(|e| ClaimError::new("account_token", e))?;
    if returned as usize > buffer.len() * size_of::<usize>()
        || (returned as usize) < size_of::<TOKEN_USER>()
    {
        return Err(ClaimError::invalid("account_token_bound"));
    }
    // SAFETY: A successful token query returned at least the aligned TOKEN_USER header.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let start = buffer.as_ptr() as usize;
    let end = start + returned as usize;
    let sid = user.User.Sid;
    let pointer = sid.0 as usize;
    if pointer < start || pointer + 8 > end {
        return Err(ClaimError::invalid("account_sid"));
    }
    // SAFETY: The fixed SID header was checked inside the live token buffer.
    let authorities = unsafe { sid.0.cast::<u8>().add(1).read() } as usize;
    if authorities > 15 || pointer + 8 + 4 * authorities > end {
        return Err(ClaimError::invalid("account_sid_bound"));
    }
    // SAFETY: Every subauthority declared by the header lies inside this token buffer.
    if !unsafe { IsValidSid(sid) }.as_bool() {
        return Err(ClaimError::invalid("account_sid"));
    }
    // SAFETY: The complete SID was validated while the token buffer remains live.
    let length = unsafe { GetLengthSid(sid) } as usize;
    if length > SECURITY_MAX_SID_SIZE as usize || pointer + length > end {
        return Err(ClaimError::invalid("account_sid_bound"));
    }
    // SAFETY: The complete checked byte range remains inside the aligned token allocation.
    Ok(unsafe { std::slice::from_raw_parts(sid.0.cast::<u8>(), length) }.to_vec())
}
