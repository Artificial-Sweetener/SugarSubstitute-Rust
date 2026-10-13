// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Perform profile filesystem effects under the shared protected-storage owner.
#[cfg(test)]
mod tests;
use super::*;
use crate::{
    ProtectedStorageRoot,
    native_claims::files::Budget,
    protected_storage::{exclusion::HeldFile, paths, record_file},
};
use std::{os::windows::ffi::OsStringExt, path::Path};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, Moment};

pub(super) struct NativeState {
    pub(super) root: Option<ProtectedStorageRoot>,
    pub(super) file: Option<HeldFile<160>>,
    pub(super) record: Option<record::Record>,
    pub(super) original_released: Option<[u8; 160]>,
    pub(super) folder: Option<Vec<u16>>,
    pub(super) held: bool,
    pub(super) retired: bool,
    pub(super) release_published: bool,
    pub(super) close_deadline: Option<Moment>,
    pub(super) prepared_folder: Option<PreparedFolder>,
    pub(super) environment: Option<(attempt::Binding, JobId)>,
    pub(super) browser: Option<(
        attempt::Binding,
        JobId,
        crate::native_process::Identity,
        crate::registry::profile::ProfileFamily,
    )>,
    pub(super) failure: Option<ClaimError>,
    // Retained allocations and proofs drop before their byte charge.
    pub(super) memory: Option<Arc<MemoryClaim>>,
}
impl NativeState {
    pub(super) fn new() -> Self {
        Self {
            root: None,
            file: None,
            memory: None,
            record: None,
            original_released: None,
            folder: None,
            held: false,
            retired: true,
            release_published: false,
            close_deadline: None,
            environment: None,
            prepared_folder: None,
            browser: None,
            failure: None,
        }
    }
}
pub(super) struct PreparedFolder {
    pub(super) binding: attempt::Binding,
    pub(super) job: JobId,
    pub(super) units: Vec<u16>,
    pub(super) memory: MemoryClaim,
}
pub(super) trait ProfilePort: Send + Sync {
    fn acquire(
        &self,
        state: &mut NativeState,
        path: &Path,
        run: RunKey,
        context: &JobContext,
    ) -> Result<(), ClaimError>;
    fn validate_environment(
        &self,
        state: &mut NativeState,
        observed: &[u16],
        context: &JobContext,
    ) -> Result<(), ClaimError>;
    fn release(&self, state: &mut NativeState, context: &JobContext) -> Result<(), ClaimError>;
}
pub(super) struct WindowsProfilePort;
impl ProfilePort for WindowsProfilePort {
    fn acquire(
        &self,
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
            account: {
                use sha2::{Digest, Sha256};
                let mut hash = Sha256::new();
                for part in [b"SugarSubstitute.profile-account.v1".as_slice(), sid] {
                    hash.update((part.len() as u64).to_le_bytes());
                    hash.update(part);
                }
                hash.finalize().into()
            },
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
            &mut state.file,
            crate::protected_storage::native_name::NativeName::Ordinary(exclusion_path),
            crate::protected_storage::native_name::NativeName::Ordinary(record_path),
            crate::protected_storage::native_name::NativeSid::Ordinary(sid.to_vec()),
            budget.clone(),
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(&budget)?,
        )?;
        check(context)?;
        let file = state.file.as_ref().ok_or_else(|| ClaimError::invalid("profile_exclusion"))?;
        let previous = match record_file::read_shape(file, context)? {
            record_file::RecordRead::Missing => None,
            record_file::RecordRead::Empty => {
                return Err(ClaimError::invalid("empty_profile_record"));
            }
            record_file::RecordRead::Present(bytes) => {
                Some((record::Record::decode(&bytes)?, bytes))
            }
        };
        if !match previous {
            Some((old, _)) => old.admits_successor(record),
            None => file.created,
        } {
            return Err(ClaimError::invalid("profile_unresolved_predecessor"));
        }
        state.original_released = previous.map(|(_, bytes)| bytes);
        state.record = Some(record);
        record_file::publish(
            file,
            &record.encode(),
            record_file::PublicationAuthority::Ordinary,
            context,
            |old| {
                if match old {
                    record_file::RecordRead::Present(bytes) => {
                        !record::Record::decode(bytes)?.permits_replacement(record)
                    }
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
    fn validate_environment(
        &self,
        state: &mut NativeState,
        observed: &[u16],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        check(context)?;
        if observed.is_empty() || observed.len() > 32766 || observed.contains(&0) || !state.held {
            return Err(ClaimError::invalid("profile_environment_shape"));
        }
        let path = PathBuf::from(std::ffi::OsString::from_wide(observed));
        let file = state.file.as_ref().ok_or_else(|| ClaimError::invalid("profile_exclusion"))?;
        let mut retained = lock(&file.temporary_handles);
        if retained.len() >= 16 {
            return Err(ClaimError::invalid("profile_validation_handle_bound"));
        }
        retained
            .try_reserve_exact(1)
            .map_err(|e| ClaimError::new("profile_validation_handle", e))?;
        let index = paths::open_retained(
            &path,
            true,
            true,
            file.handle.budget(),
            &mut retained,
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &(file.handle.budget()),
            )?,
        )?;
        let result = (|| {
            let actual = paths::file_identity(
                &retained[index],
                &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                    &retained[index].budget(),
                )?,
            )?;
            let expected =
                state.record.ok_or_else(|| ClaimError::invalid("profile_identity"))?.folder;
            if actual != expected {
                return Err(ClaimError::invalid("profile_environment_mismatch"));
            }
            crate::protected_storage::security::verify(
                &retained[index],
                file.account_sid.bytes()?,
                &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                    &retained[index].budget(),
                )?,
            )?;
            check(context)
        })();
        let cleanup = retained[index].close();
        if cleanup.is_ok() {
            retained.remove(index);
        }
        ClaimError::finish(result, cleanup)
    }
    fn release(&self, state: &mut NativeState, context: &JobContext) -> Result<(), ClaimError> {
        check(context)?;
        if let Some(file) = state.file.as_ref().filter(|file| file.is_locked()) {
            let expected = state.record.map(record::Record::encode);
            let observed = record_file::retire_publication(file, expected.as_ref(), context)?;
            if prepare_release(observed, &mut state.record, state.original_released.as_ref())? {
                let record =
                    state.record.ok_or_else(|| ClaimError::invalid("profile_release_record"))?;
                record_file::publish(
                    file,
                    &record.encode(),
                    record_file::PublicationAuthority::Ordinary,
                    context,
                    |old| match old {
                        record_file::RecordRead::Present(bytes)
                            if record::Record::decode(bytes)?.permits_replacement(record) =>
                        {
                            Ok(())
                        }
                        _ => Err(ClaimError::invalid("profile_release_changed_record")),
                    },
                )?;
            }
            state.release_published = true;
        }
        check(context)?;
        if let Some(file) = state.file.as_mut() {
            file.close()?;
        }
        state.file = None;
        state.held = false;
        if let Some(root) = state.root.as_mut() {
            root.retire(context)?;
        }
        state.root = None;
        state.held = false;
        state.retired = true;
        Ok(())
    }
}
pub(super) fn check(context: &JobContext) -> Result<(), ClaimError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|limit| context.now() >= limit)
    {
        Err(ClaimError::invalid("profile_context_expired"))
    } else {
        Ok(())
    }
}

/// Prove actual profile ownership before retaining a new Released generation.
/// # Errors
/// Reject corrupt or changed ownership without altering the retained generation.
fn prepare_release(
    observed: record_file::PublicationRetirement<160>,
    expected: &mut Option<record::Record>,
    original_released: Option<&[u8; 160]>,
) -> Result<bool, ClaimError> {
    let bytes = match observed {
        record_file::PublicationRetirement::Preserved(
            record_file::RecordRead::Missing | record_file::RecordRead::Empty,
        ) => return Ok(false),
        record_file::PublicationRetirement::Preserved(record_file::RecordRead::Present(bytes))
        | record_file::PublicationRetirement::Activated(bytes) => bytes,
    };
    let actual = record::Record::decode(&bytes)?;
    if original_released == Some(&bytes) && actual.state == record::State::Released {
        return Ok(false);
    }
    let own = expected.ok_or_else(|| ClaimError::invalid("profile_retirement_generation"))?;
    if actual.account != own.account || actual.folder != own.folder || actual.run != own.run {
        return Err(ClaimError::invalid("changed_profile_retirement_generation"));
    }
    let released = record::Record { state: record::State::Released, ..actual };
    let needed = bytes != released.encode();
    *expected = Some(released);
    Ok(needed)
}
