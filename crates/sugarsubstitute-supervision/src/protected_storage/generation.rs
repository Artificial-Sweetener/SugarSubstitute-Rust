// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exclusive generation activation uses the retained validated parent chain.
use super::*;
use std::os::windows::ffi::OsStrExt;
use windows::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FILE_CREATE, FILE_DIRECTORY_FILE, FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
        },
    },
    Win32::{
        Foundation::{
            HANDLE, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, RtlNtStatusToDosError, UNICODE_STRING,
        },
        Storage::FileSystem::{
            FILE_ATTRIBUTE_DIRECTORY, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
            FILE_SHARE_WRITE, FILE_TRAVERSE, READ_CONTROL, SYNCHRONIZE,
        },
        System::IO::IO_STATUS_BLOCK,
    },
    core::PWSTR,
};

fn name(coordinator: CoordinatorKey) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(107);
    result.push_str("capture-v1-");
    for byte in coordinator.namespace.bytes().iter().chain(coordinator.epoch.bytes()) {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    result
}
fn create_new(
    state: &mut State,
    path: &Path,
    parent: usize,
    context: &JobContext,
) -> Result<usize, ClaimError> {
    check(context)?;
    paths::wide(path)?;
    if state.handles.len() == HANDLE_BOUND {
        return Err(ClaimError::invalid("storage_handle_bound"));
    }
    let child = path.file_name().ok_or_else(|| ClaimError::invalid("generation_child_name"))?;
    let mut units: Vec<u16> = child.encode_wide().collect();
    let length =
        u16::try_from(units.len() * 2).map_err(|e| ClaimError::new("generation_child_bound", e))?;
    let object_name =
        UNICODE_STRING { Length: length, MaximumLength: length, Buffer: PWSTR(units.as_mut_ptr()) };
    let descriptor = security::Descriptor::new(&state.sid)?;
    let attributes = descriptor.attributes();
    let object = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: state.handles[parent].raw(),
        ObjectName: &object_name,
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        SecurityDescriptor: attributes.lpSecurityDescriptor.cast(),
        ..Default::default()
    };
    let mut raw = HANDLE::default();
    let mut io = IO_STATUS_BLOCK::default();
    // SAFETY: Fixed one-component relative name, actual retained parent and aligned
    // descriptor remain live. Synchronous FILE_CREATE atomically returns the created
    // directory handle; an existing name fails. No handle inherits or shares deletion.
    let status = unsafe {
        NtCreateFile(
            &mut raw,
            FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY | FILE_TRAVERSE | READ_CONTROL | SYNCHRONIZE,
            &object,
            &mut io,
            None,
            FILE_ATTRIBUTE_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_CREATE,
            FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
            None,
            0,
        )
    };
    let budget = files::Budget::Protected(state.memory.clone());
    let index = state.handles.len();
    if !raw.0.is_null() {
        state.handles.push(Handle::new(raw, budget));
    }
    if status.0 != 0 {
        // SAFETY: Pure conversion of this immediate native status preserves its error code.
        let code = unsafe { RtlNtStatusToDosError(status) };
        return Err(ClaimError::new(
            "create_capture_generation",
            std::io::Error::from_raw_os_error(code as i32),
        ));
    }
    // FILE_CREATED=2 is the pinned SDK result; success never substitutes existing state.
    if raw.0.is_null() || io.Information != 2 {
        return Err(ClaimError::invalid("generation_creation_output"));
    }
    paths::validate_object(&state.handles[index], true, true)?;
    let identity = paths::file_identity(&state.handles[index])?;
    security::verify(&state.handles[index], &state.sid)?;
    if paths::final_path(&state.handles[index])? != path {
        return Err(ClaimError::invalid("capture_generation_resolution"));
    }
    paths::verify_pinned(
        path,
        identity,
        &mut state.handles,
        true,
        files::Budget::Protected(state.memory.clone()),
    )?;
    check(context)?;
    Ok(index)
}
pub(super) fn create(
    state: &mut State,
    coordinator: CoordinatorKey,
    context: &JobContext,
) -> Result<(), ClaimError> {
    bind_context(state, context, false)?;
    check(context)?;
    if state.attempted {
        return Err(ClaimError::invalid("generation_already_attempted"));
    }
    let (root, identity, root_index) =
        state.root.as_ref().ok_or_else(|| ClaimError::invalid("missing_protected_root"))?;
    let root_index = *root_index;
    let directory = root.join(name(coordinator));
    let database = directory.join("database");
    let metrics = directory.join("metrics");
    for path in [&directory, &database, &metrics] {
        paths::wide(path)?;
    }
    paths::verify_pinned(
        root,
        *identity,
        &mut state.handles,
        true,
        files::Budget::Protected(state.memory.clone()),
    )?;
    // Retain all intended generation paths before the first create-new effect.
    state.attempted = true;
    state.generation = Some(ProtectedStorageGeneration {
        directory: directory.clone(),
        database: database.clone(),
        metrics: metrics.clone(),
        ready: false,
    });
    let parent = create_new(state, &directory, root_index, context)?;
    create_new(state, &database, parent, context)?;
    create_new(state, &metrics, parent, context)?;
    state
        .generation
        .as_mut()
        .ok_or_else(|| ClaimError::invalid("missing_capture_generation"))?
        .ready = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generation_name_binds_exact_namespace_epoch_and_has_no_path_components()
    -> Result<(), Box<dyn std::error::Error>> {
        use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, NamespaceId};
        let key = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        };
        assert_eq!(name(key), format!("capture-v1-{}{}", "01".repeat(32), "02".repeat(16)));
        assert_eq!(Path::new(&name(key)).components().count(), 1);
        let other = CoordinatorKey { epoch: CoordinatorEpoch::new([3; 16])?, ..key };
        assert_ne!(name(key), name(other));
        Ok(())
    }
}
