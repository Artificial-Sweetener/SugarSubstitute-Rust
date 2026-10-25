// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Perform profile filesystem effects under the shared protected-storage owner.
mod acquisition;
mod disposal;
mod recovery;
#[cfg(test)]
mod tests;
use super::*;
use crate::{
    ProtectedStorageRoot,
    durable_storage::protected::{exclusion::HeldFile, paths, record_file},
    native_claims::files::Budget,
};
use std::{os::windows::ffi::OsStringExt, path::Path};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, Moment};

pub(super) struct NativeState {
    pub(super) root: Option<ProtectedStorageRoot>,
    pub(super) file: Option<HeldFile<160>>,
    pub(super) record: Option<record::Record>,
    pub(super) predecessor: Option<[u8; 160]>,
    pub(super) folder: Option<Vec<u16>>,
    pub(super) held: bool,
    pub(super) retired: bool,
    pub(super) release_published: bool,
    pub(super) publication_attempted: bool,
    pub(super) desktop_custody: Option<crate::registry::profile::prepared::PreparedDesktop>,
    pub(super) custody_record: Option<record::custody::CustodiedRecord>,
    pub(super) custody_release: Option<super::custody::Release>,
    pub(super) migration_file: Option<HeldFile<160>>,
    recovery: recovery::Recovery,
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
            predecessor: None,
            folder: None,
            held: false,
            retired: true,
            release_published: false,
            publication_attempted: false,
            desktop_custody: None,
            custody_record: None,
            custody_release: None,
            migration_file: None,
            recovery: recovery::Recovery::default(),
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
        acquisition::acquire(state, path, run, context)
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
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                &(file.handle.budget()),
            )?,
        )?;
        let result = (|| {
            let actual = paths::file_identity(
                &retained[index],
                &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                    &retained[index].budget(),
                )?,
            )?;
            let expected =
                state.record.ok_or_else(|| ClaimError::invalid("profile_identity"))?.folder;
            if actual != expected {
                return Err(ClaimError::invalid("profile_environment_mismatch"));
            }
            crate::durable_storage::protected::security::verify(
                &retained[index],
                file.account_sid.bytes()?,
                &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
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
        if let Some(prepared) = state.desktop_custody.as_mut().filter(|prepared| {
            prepared.prepared_nonce.is_none() && prepared.queued_prepare.is_none()
        }) {
            prepared
                .close_unissued(context)
                .map_err(|cause| ClaimError::new("profile_prepared_cleanup", cause))?;
            state.desktop_custody = None;
        }
        if let Some(file) = &mut state.migration_file {
            let original = state
                .predecessor
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("profile_migration_original_missing"))?;
            record_file::retire_publication(file, Some(original), context)?;
            file.close()?;
            state.migration_file = None;
        }
        state.recovery.retire(context)?;
        if disposal::dispose_unpublished(
            state,
            context,
            |file| file.close(),
            |root| root.retire(context),
        )? {
            return Ok(());
        }
        if let Some(file) = state.file.as_ref().filter(|file| file.is_locked()) {
            if let Some(held) = state.custody_record {
                let encoded = held.encode();
                let released = held.released().encode();
                let observed = record_file::retire_publication(file, Some(&encoded), context)?;
                match observed {
                    record_file::PublicationRetirement::Preserved(
                        record_file::RecordRead::Present(bytes),
                    )
                    | record_file::PublicationRetirement::Activated(bytes)
                        if bytes == encoded || bytes == released =>
                    {
                        if bytes != released {
                            record_file::publish(
                                file,
                                &released,
                                record_file::PublicationAuthority::Ordinary,
                                context,
                                |old| match old {
                                    record_file::RecordRead::Present(actual)
                                        if *actual == encoded =>
                                    {
                                        Ok(())
                                    }
                                    _ => Err(ClaimError::invalid(
                                        "profile_custody_release_changed_record",
                                    )),
                                },
                            )?;
                        }
                        state.custody_record = Some(held.released());
                    }
                    record_file::PublicationRetirement::Preserved(
                        record_file::RecordRead::Missing,
                    ) if !state.held => {}
                    record_file::PublicationRetirement::Preserved(
                        record_file::RecordRead::Present(bytes),
                    ) if !state.held && state.predecessor == Some(bytes) => {}
                    _ => return Err(ClaimError::invalid("profile_custody_release_identity")),
                }
            } else {
                let expected = state.record.map(record::Record::encode);
                let observed = record_file::retire_publication(file, expected.as_ref(), context)?;
                if prepare_release(observed, &mut state.record, state.predecessor.as_ref())? {
                    let record = state
                        .record
                        .ok_or_else(|| ClaimError::invalid("profile_release_record"))?;
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
    predecessor: Option<&[u8; 160]>,
) -> Result<bool, ClaimError> {
    let bytes = match observed {
        record_file::PublicationRetirement::Preserved(
            record_file::RecordRead::Missing | record_file::RecordRead::Empty,
        ) => return Ok(false),
        record_file::PublicationRetirement::Preserved(record_file::RecordRead::Present(bytes))
        | record_file::PublicationRetirement::Activated(bytes) => bytes,
    };
    if predecessor == Some(&bytes) {
        return Ok(false);
    }
    let actual = record::Record::decode(&bytes)?;
    let own = expected.ok_or_else(|| ClaimError::invalid("profile_retirement_generation"))?;
    if actual.account != own.account || actual.folder != own.folder || actual.run != own.run {
        return Err(ClaimError::invalid("changed_profile_retirement_generation"));
    }
    let released = record::Record { state: record::State::Released, ..actual };
    let needed = bytes != released.encode();
    *expected = Some(released);
    Ok(needed)
}
