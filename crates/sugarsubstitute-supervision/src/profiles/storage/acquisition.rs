// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acquire one profile while preserving native predecessor exclusion and recovery.
use super::*;

/// Resolve and lock the profile before admitting its next generation.
pub(super) fn acquire(
    state: &mut NativeState,
    path: &Path,
    run: RunKey,
    context: &JobContext,
) -> Result<(), ClaimError> {
    check(context)?;
    let root = ProtectedStorageRoot::new(
        context
            .memory()
            .claim(1024 * 1024)
            .map_err(|e| ClaimError::new("profile_storage_claim", e))?,
    )?;
    state.root = Some(root);
    state.retired = false;
    state.memory = Some(Arc::new(
        context
            .memory()
            .claim(512 * 1024)
            .map_err(|e| ClaimError::new("profile_record_claim", e))?,
    ));
    let root = state.root.as_mut().ok_or_else(|| ClaimError::invalid("profile_storage"))?;
    root.resolve_or_create_profile_leaf(path, context)?;
    let (folder, identity, sid) = root.profile_folder()?;
    let record = record::Record {
        account: record::account_identity(sid),
        folder: identity,
        run,
        state: record::State::Held,
    };
    let units: Vec<u16> = {
        use std::os::windows::ffi::OsStrExt;
        folder.as_os_str().encode_wide().collect()
    };
    if units.capacity() > 32766 {
        return Err(ClaimError::invalid("profile_folder_capacity"));
    }
    state.folder = Some(units);
    let record_path = folder.join(".sugarsubstitute-profile-v1.record");
    let exclusion_path = folder.join(".sugarsubstitute-profile-v1.claim");
    check(context)?;
    let budget = Budget::Protected(
        state.memory.as_ref().ok_or_else(|| ClaimError::invalid("profile_memory"))?.clone(),
    );
    HeldFile::acquire_into(
        crate::durable_storage::protected::exclusion::ExclusionOpen::CreateOrOpen,
        &mut state.file,
        crate::durable_storage::protected::native_name::NativeName::Ordinary(exclusion_path),
        crate::durable_storage::protected::native_name::NativeName::Ordinary(record_path),
        crate::durable_storage::protected::native_name::NativeSid::Ordinary(sid.to_vec()),
        budget.clone(),
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(&budget)?,
    )
    .map_err(|cause| {
        super::super::contention::at_profile(cause, folder, identity, record.account)
    })?;
    check(context)?;
    let file = state.file.as_ref().ok_or_else(|| ClaimError::invalid("profile_exclusion"))?;
    let previous = match record_file::read_shape(file, context)? {
        record_file::RecordRead::Missing => None,
        record_file::RecordRead::Empty => {
            return Err(ClaimError::invalid("empty_profile_record"));
        }
        record_file::RecordRead::Present(bytes) => {
            let old = if bytes[8..10] == [2, 0] {
                record::custody::CustodiedRecord::decode(&bytes)?.original
            } else {
                record::Record::decode(&bytes)?
            };
            Some((old, bytes))
        }
    };
    if let Some((old, bytes)) = previous
        && old.state == record::State::Held
        && bytes[8..10] == [2, 0]
        && old.same_store(record)
    {
        state.recovery.recover(file, folder, sid, &bytes, budget.clone(), context)?;
    }
    if !match previous {
        Some((old, _)) => old.admits_successor(record) || state.recovery.admitted(),
        None => file.created,
    } {
        return Err(ClaimError::invalid("profile_unresolved_predecessor"));
    }
    state.predecessor = previous.map(|(_, bytes)| bytes);
    if state.desktop_custody.is_some() {
        super::custody::preparation::prepare(state, record, context)?;
        return check(context);
    }
    state.record = Some(record);
    state.publication_attempted = true;
    record_file::publish(
        file,
        &record.encode(),
        record_file::PublicationAuthority::Ordinary,
        context,
        |old| {
            if match old {
                record_file::RecordRead::Present(bytes) => Some(*bytes) != state.predecessor,
                record_file::RecordRead::Missing => false,
                record_file::RecordRead::Empty => true,
            } {
                Err(ClaimError::invalid("profile_changed_record"))
            } else {
                Ok(())
            }
        },
    )?;
    state.held = true;
    check(context)
}
