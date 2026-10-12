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
    root.sid = crate::protected_storage::account::token_sid(token)?;
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
    let epoch = sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new(
        crate::protected_storage::record_file::random()?,
    )
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
    let stem = directory
        .join("coordinators")
        .join(crate::protected_storage::record_file::hex(namespace.bytes()));
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
