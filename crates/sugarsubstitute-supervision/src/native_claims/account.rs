// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive namespace and protected storage from the primary token and pinned installation.
use super::*;
use windows::Win32::{
    Foundation::HANDLE,
    Security::*,
    System::{
        Com::CoTaskMemFree,
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
    UI::Shell::*,
};
fn token_sid(token: HANDLE) -> Result<Vec<u8>, ClaimError> {
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
pub(crate) fn initialize(root: &mut Root, installation: &Path) -> Result<(), ClaimError> {
    let pool = root.pool.as_ref().ok_or_else(|| ClaimError::invalid("missing_pool"))?.clone();
    let budget = files::Budget::Core(pool);
    let mut token_raw = HANDLE::default();
    // SAFETY: The pseudo-process handle is borrowed; token_raw is writable owned output.
    let opened = unsafe {
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_IMPERSONATE, &mut token_raw)
    }
    .map_err(|e| ClaimError::new("primary_token", e));
    if !token_raw.0.is_null() {
        root.handles.push(Handle::new(token_raw, budget.clone()));
    }
    opened?;
    if token_raw.0.is_null() {
        return Err(ClaimError::invalid("missing_primary_token"));
    }
    let token = root.handles[0].raw();
    root.sid = token_sid(token)?;
    let installed =
        paths::open_retained(installation, true, false, budget.clone(), &mut root.handles)?;
    let installed_path = paths::final_path(&root.handles[installed])?;
    let installed_identity = paths::file_identity(&root.handles[installed])?;
    paths::pin_ancestors(&installed_path, &mut root.handles, budget.clone())?;
    paths::verify_pinned(
        &installed_path,
        installed_identity,
        &mut root.handles,
        true,
        budget.clone(),
    )?;
    let namespace = sugarsubstitute_contracts::lifecycle::NamespaceId::new(paths::hash(&[
        b"SugarSubstitute.coordinator-namespace.v1",
        &root.sid,
        &installed_identity,
    ]));
    let epoch = sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new(records::random()?)
        .map_err(|e| ClaimError::new("coordinator_epoch", e))?;
    let key = CoordinatorKey { namespace, epoch };
    root.key = Some(key);
    // SAFETY: The retained primary token has QUERY|IMPERSONATE access; the GUID is static.
    let path =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KNOWN_FOLDER_FLAG(0), Some(token)) }
            .map_err(|e| ClaimError::new("account_storage", e))?;
    // SAFETY: Shell returns a valid NUL-terminated UTF-16 allocation on success.
    let text = unsafe { security::bounded_text(path, 32767) };
    // SAFETY: This exact Shell allocation is released once using its documented allocator.
    unsafe { CoTaskMemFree(Some(path.0.cast())) };
    let known = PathBuf::from(text?);
    let known_handle =
        paths::open_retained(&known, true, false, budget.clone(), &mut root.handles)?;
    let mut directory = paths::final_path(&root.handles[known_handle])?;
    let known_identity = paths::file_identity(&root.handles[known_handle])?;
    paths::pin_ancestors(&directory, &mut root.handles, budget.clone())?;
    paths::verify_pinned(&directory, known_identity, &mut root.handles, true, budget.clone())?;
    for component in ["SugarSubstitute", "runtime", "claims"] {
        directory.push(component);
        paths::create_directory(&directory, &root.sid, budget.clone(), &mut root.handles)?;
    }
    root.directory = directory.clone();
    for component in ["coordinators", "backends"] {
        paths::create_directory(
            &directory.join(component),
            &root.sid,
            budget.clone(),
            &mut root.handles,
        )?;
    }
    let stem = directory.join("coordinators").join(records::hex(namespace.bytes()));
    HeldFile::acquire_into(
        &mut root.coordinator,
        &stem.with_extension("claim"),
        stem.with_extension("record"),
        &root.sid,
        budget,
    )?;
    let held = root.coordinator.as_ref().ok_or_else(|| ClaimError::invalid("missing_claim"))?;
    let record = Record::coordinator(key);
    records::check_predecessor(held, &root.sid, &record)?;
    root.record = Some(record);
    records::publish(
        held,
        root.record.as_ref().ok_or_else(|| ClaimError::invalid("missing_record"))?,
    )?;
    root.claimed = true;
    Ok(())
}
