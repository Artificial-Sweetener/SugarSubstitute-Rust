// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve actual primary-account and pinned installation namespace authority.
use super::paths;
use crate::{
    ClaimError,
    native_claims::files::{Budget, Handle},
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use sugarsubstitute_contracts::lifecycle::NamespaceId;
use windows::Win32::{
    Foundation::HANDLE,
    Security::*,
    System::{
        Com::CoTaskMemFree,
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
    UI::Shell::*,
};

/// Preserve the minimum primary-token rights required by the actual consumer.
pub(crate) enum PrimaryAccountAccess {
    Query,
    AccountStorage,
}
/// Observe one actual retained token and its bounded account/session facts.
pub(crate) struct CurrentAccount {
    pub(crate) token_slot: usize,
    pub(crate) sid: Vec<u8>,
    pub(crate) session_id: u32,
}
/// Retain actual primary-token output before inspecting native status.
/// Errors retain partial token custody in the caller's charged handle owner.
pub(crate) fn resolve_current(
    handles: &mut Vec<Handle>,
    budget: Budget,
    access: PrimaryAccountAccess,
) -> Result<CurrentAccount, ClaimError> {
    handles.try_reserve_exact(1).map_err(|e| ClaimError::new("account_handle_reservation", e))?;
    let rights = match access {
        PrimaryAccountAccess::Query => TOKEN_QUERY,
        PrimaryAccountAccess::AccountStorage => TOKEN_QUERY | TOKEN_IMPERSONATE,
    };
    let mut raw = HANDLE::default();
    // SAFETY: Borrowed pseudo-process handle and initialized token output are valid.
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), rights, &mut raw) }
        .map_err(|e| ClaimError::new("primary_token", e));
    let token_slot = handles.len();
    if !raw.0.is_null() {
        handles.push(Handle::new(raw, budget));
    }
    opened?;
    let token = handles
        .get(token_slot)
        .filter(|_| !raw.0.is_null())
        .ok_or_else(|| ClaimError::invalid("missing_primary_token"))?
        .raw();
    let sid = token_sid(token)?;
    let mut session_id = 0_u32;
    let mut returned = 0;
    // SAFETY: This initialized DWORD exactly matches TokenSessionId's documented output.
    unsafe {
        GetTokenInformation(
            token,
            TokenSessionId,
            Some((&mut session_id as *mut u32).cast()),
            size_of::<u32>() as u32,
            &mut returned,
        )
    }
    .map_err(|e| ClaimError::new("account_session", e))?;
    if returned as usize != size_of::<u32>() {
        return Err(ClaimError::invalid("account_session_bound"));
    }
    Ok(CurrentAccount { token_slot, sid, session_id })
}
/// Observe canonical account storage bound to one actual pinned installation.
pub(crate) struct ResolvedNamespace {
    pub(crate) namespace: NamespaceId,
    pub(crate) account_sid: Vec<u8>,
    pub(crate) session_id: u32,
    pub(crate) installation_identity: [u8; 24],
    pub(crate) directory: PathBuf,
}
/// Resolve existing namespace inputs and protected claims directories under retained custody.
/// Errors preserve every partial token/path handle in the original charged owner.
pub(crate) fn resolve_namespace(
    installation: &Path,
    handles: &mut Vec<Handle>,
    budget: Budget,
) -> Result<ResolvedNamespace, ClaimError> {
    let account = resolve_current(handles, budget.clone(), PrimaryAccountAccess::AccountStorage)?;
    let token = handles[account.token_slot].raw();
    let installed = paths::open_retained(installation, true, false, budget.clone(), handles)?;
    let installed_path = paths::final_path(&handles[installed])?;
    let installation_identity = paths::file_identity(&handles[installed])?;
    paths::pin_ancestors(&installed_path, handles, budget.clone())?;
    paths::verify_pinned(&installed_path, installation_identity, handles, true, budget.clone())?;
    let namespace = namespace_identity(&account.sid, &installation_identity);
    // SAFETY: Retained QUERY|IMPERSONATE token and static known-folder GUID remain valid.
    let raw =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KNOWN_FOLDER_FLAG(0), Some(token)) }
            .map_err(|e| ClaimError::new("account_storage", e))?;
    // SAFETY: Successful Shell output is a valid bounded NUL-terminated allocation.
    let known = unsafe { paths::read_native_path(raw, 32767) };
    // SAFETY: This exact Shell allocation is released once with its documented allocator.
    unsafe { CoTaskMemFree(Some(raw.0.cast())) };
    let known = known?;
    let known_handle = paths::open_retained(&known, true, false, budget.clone(), handles)?;
    let mut directory = paths::final_path(&handles[known_handle])?;
    let known_identity = paths::file_identity(&handles[known_handle])?;
    paths::pin_ancestors(&directory, handles, budget.clone())?;
    paths::verify_pinned(&directory, known_identity, handles, true, budget.clone())?;
    for component in ["SugarSubstitute", "runtime", "claims"] {
        directory.push(component);
        paths::create_directory(&directory, &account.sid, budget.clone(), handles)?;
    }
    for component in ["coordinators", "backends"] {
        paths::create_directory(&directory.join(component), &account.sid, budget.clone(), handles)?;
    }
    Ok(ResolvedNamespace {
        namespace,
        account_sid: account.sid,
        session_id: account.session_id,
        installation_identity,
        directory,
    })
}
fn namespace_identity(sid: &[u8], installation: &[u8; 24]) -> NamespaceId {
    NamespaceId::new(hash_parts(&[b"SugarSubstitute.coordinator-namespace.v1", sid, installation]))
}
fn hash_parts(parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn namespace_hash_has_domain_and_part_boundaries() {
        assert_ne!(
            hash_parts(&[b"namespace.v1", b"a", b"bc"]),
            hash_parts(&[b"namespace.v1", b"ab", b"c"])
        );
        assert_ne!(hash_parts(&[b"namespace.v1", b"a"]), hash_parts(&[b"backend.v1", b"a"]));
    }
    #[test]
    fn namespace_identity_preserves_account_and_actual_installation_binding() {
        let identity = namespace_identity(&[1, 2, 3], &[4; 24]);
        assert_eq!(
            identity.bytes(),
            &hash_parts(&[b"SugarSubstitute.coordinator-namespace.v1", &[1, 2, 3], &[4; 24]])
        );
        assert_ne!(identity, namespace_identity(&[1, 2, 9], &[4; 24]));
        assert_ne!(identity, namespace_identity(&[1, 2, 3], &[9; 24]));
    }
}
