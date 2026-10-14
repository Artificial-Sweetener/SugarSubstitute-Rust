// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve actual primary-account and pinned installation namespace authority.
mod namespace_directory;
use super::{
    native_name::{NativeSid, NativeSidDestination},
    native_phase::NativePhase,
    paths,
};
use crate::claim_error::BoundedNativeCause;
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
    pub(crate) sid: NativeSid,
    pub(crate) session_id: u32,
}
/// Retain actual primary-token output before inspecting native status.
/// Errors retain partial token custody in the caller's charged handle owner.
pub(crate) fn resolve_current(
    handles: &mut Vec<Handle>,
    budget: Budget,
    access: PrimaryAccountAccess,
    phase: &mut NativePhase<'_>,
) -> Result<CurrentAccount, ClaimError> {
    phase.check_budget(&budget)?;
    phase.account()?;
    if !budget.is_ordinary() && matches!(access, PrimaryAccountAccess::AccountStorage) {
        return Err(budget.rejection("diagnostic_account_storage_access"));
    }
    if handles.len() == handles.capacity() {
        if !budget.is_ordinary() {
            return Err(budget.rejection("account_handle_capacity"));
        }
        handles.try_reserve_exact(1).map_err(|cause| {
            phase.error("account_handle_reservation", BoundedNativeCause::Reserve(cause))
        })?;
    }
    let token_slot = handles.len();
    handles.push(Handle::new(HANDLE::default(), budget));
    let (sid, session_id) = resolve_current_cell(&mut handles[token_slot], access, phase)?;
    Ok(CurrentAccount { token_slot, sid, session_id })
}
/// Observe the sole primary-token algorithm in an already retained physical cell.
/// The selected caller supplies its genuine pre-reserved cell; this method creates
/// no credit and adopts partial output before inspecting status or account facts.
pub(crate) fn resolve_current_cell(
    cell: &mut Handle,
    access: PrimaryAccountAccess,
    phase: &mut NativePhase<'_>,
) -> Result<(NativeSid, u32), ClaimError> {
    phase.check_budget(&cell.budget())?;
    phase.account()?;
    if !cell.budget().is_ordinary() && matches!(access, PrimaryAccountAccess::AccountStorage) {
        return Err(cell.budget().rejection("account_storage_access"));
    }
    let destination = phase.sid_destination()?;
    let rights = match access {
        PrimaryAccountAccess::Query => TOKEN_QUERY,
        PrimaryAccountAccess::AccountStorage => TOKEN_QUERY | TOKEN_IMPERSONATE,
    };
    let mut raw = HANDLE::default();
    phase.check()?;
    // SAFETY: Borrowed pseudo-process handle and initialized token output are valid.
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), rights, &mut raw) };
    cell.adopt_output(raw);
    opened.map_err(|cause| phase.error("primary_token", BoundedNativeCause::Windows(cause)))?;
    if raw.0.is_null() {
        return Err(phase.invalid("missing_primary_token"));
    }
    let token = cell.raw();
    let sid = token_sid(token, phase, destination)?;
    let mut session_id = 0_u32;
    let mut returned = 0;
    phase.check()?;
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
    .map_err(|cause| phase.error("account_session", BoundedNativeCause::Windows(cause)))?;
    if returned as usize != size_of::<u32>() {
        return Err(phase.invalid("account_session_bound"));
    }
    Ok((sid, session_id))
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
    let mut phase = NativePhase::ordinary(&budget)?;
    let account =
        resolve_current(handles, budget.clone(), PrimaryAccountAccess::AccountStorage, &mut phase)?;
    let token = handles[account.token_slot].raw();
    let installed = paths::open_retained(
        installation,
        true,
        false,
        budget.clone(),
        handles,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
    )?;
    let installed_path = paths::final_path(
        &handles[installed],
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(
            &handles[installed].budget(),
        )?,
        crate::protected_storage::native_name::NativeNameDestination::Ordinary,
    )
    .and_then(crate::protected_storage::native_name::NativeName::into_ordinary)?;
    let installation_identity = paths::file_identity(
        &handles[installed],
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(
            &handles[installed].budget(),
        )?,
    )?;
    paths::pin_ancestors(
        &installed_path,
        handles,
        budget.clone(),
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
    )?;
    paths::verify_pinned(
        &installed_path,
        installation_identity,
        handles,
        true,
        budget.clone(),
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
    )?;
    let namespace = namespace_identity(account.sid.bytes()?, &installation_identity);
    if budget.expected_namespace().is_some_and(|expected| expected != namespace) {
        return Err(ClaimError::invalid("custody_installation_namespace"));
    }
    // SAFETY: Retained QUERY|IMPERSONATE token and static known-folder GUID remain valid.
    let raw =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KNOWN_FOLDER_FLAG(0), Some(token)) }
            .map_err(|e| ClaimError::new("account_storage", e))?;
    // SAFETY: Successful Shell output is a valid bounded NUL-terminated allocation.
    let known = unsafe { paths::read_native_path(raw, 32767) };
    // SAFETY: This exact Shell allocation is released once with its documented allocator.
    unsafe { CoTaskMemFree(Some(raw.0.cast())) };
    let known = known?;
    let known_handle = paths::open_retained(
        &known,
        true,
        false,
        budget.clone(),
        handles,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
    )?;
    let mut directory = paths::final_path(
        &handles[known_handle],
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(
            &handles[known_handle].budget(),
        )?,
        crate::protected_storage::native_name::NativeNameDestination::Ordinary,
    )
    .and_then(crate::protected_storage::native_name::NativeName::into_ordinary)?;
    let known_identity = paths::file_identity(
        &handles[known_handle],
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(
            &handles[known_handle].budget(),
        )?,
    )?;
    paths::pin_ancestors(
        &directory,
        handles,
        budget.clone(),
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
    )?;
    paths::verify_pinned(
        &directory,
        known_identity,
        handles,
        true,
        budget.clone(),
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&(budget.clone()))?,
    )?;
    directory = namespace_directory::resolve(directory, account.sid.bytes()?, budget, handles)?;
    Ok(ResolvedNamespace {
        namespace,
        account_sid: account.sid.into_ordinary()?,
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
pub(crate) fn token_sid(
    token: HANDLE,
    phase: &mut NativePhase<'_>,
    destination: NativeSidDestination,
) -> Result<NativeSid, ClaimError> {
    phase.check_sid_destination(&destination)?;
    // TOKEN_USER and its referenced SID share this aligned, strictly bounded buffer.
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(256)
        .map_err(|cause| phase.error("account_token_buffer", BoundedNativeCause::Reserve(cause)))?;
    if buffer.capacity() > 256 {
        return Err(phase.invalid("account_token_buffer_capacity"));
    }
    buffer.resize(256, 0_usize);
    phase.account()?;
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
    .map_err(|cause| phase.error("account_token", BoundedNativeCause::Windows(cause)))?;
    if returned as usize > buffer.len() * size_of::<usize>()
        || (returned as usize) < size_of::<TOKEN_USER>()
    {
        return Err(phase.invalid("account_token_bound"));
    }
    // SAFETY: A successful token query returned at least the aligned TOKEN_USER header.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let start = buffer.as_ptr() as usize;
    let end = start + returned as usize;
    let sid = user.User.Sid;
    let pointer = sid.0 as usize;
    if pointer < start || pointer + 8 > end {
        return Err(phase.invalid("account_sid"));
    }
    // SAFETY: The fixed SID header was checked inside the live token buffer.
    let authorities = unsafe { sid.0.cast::<u8>().add(1).read() } as usize;
    if authorities > 15 || pointer + 8 + 4 * authorities > end {
        return Err(phase.invalid("account_sid_bound"));
    }
    // SAFETY: Every subauthority declared by the header lies inside this token buffer.
    if !unsafe { IsValidSid(sid) }.as_bool() {
        return Err(phase.invalid("account_sid"));
    }
    // SAFETY: The complete SID was validated while the token buffer remains live.
    let length = unsafe { GetLengthSid(sid) } as usize;
    if length > SECURITY_MAX_SID_SIZE as usize || pointer + length > end {
        return Err(phase.invalid("account_sid_bound"));
    }
    // SAFETY: The complete checked byte range remains inside the aligned token allocation.
    let actual = unsafe { std::slice::from_raw_parts(sid.0.cast::<u8>(), length) };
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(72).map_err(|cause| {
        phase.error("account_sid_destination", BoundedNativeCause::Reserve(cause))
    })?;
    if bytes.capacity() > 72 {
        return Err(phase.invalid("account_sid_capacity"));
    }
    bytes.extend_from_slice(actual);
    drop(buffer);
    destination.finish(bytes)
}

/// Describe the fixed query-token buffer and supported account phase controls.
/// Account-storage Shell allocation remains exclusively with the ordinary branch.
pub(crate) fn diagnostic_account_layout() -> Result<
    crate::native_claims::files::NativeRoleLayout,
    crate::native_claims::files::NativeLayoutFailure,
> {
    use crate::{
        diagnostic_storage::DiagnosticNativeScratchLoan,
        native_claims::files::{NativeLayoutFailure, NativeRoleLayout},
    };
    let buffer =
        std::alloc::Layout::array::<usize>(256).map_err(|_| NativeLayoutFailure::Overflow)?.size();
    let peak = buffer
        .checked_add(size_of::<Vec<usize>>())
        .and_then(|value| value.checked_add(size_of::<DiagnosticNativeScratchLoan>()))
        .ok_or(NativeLayoutFailure::Overflow)?;
    Ok(NativeRoleLayout { retained: 0, peak })
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
