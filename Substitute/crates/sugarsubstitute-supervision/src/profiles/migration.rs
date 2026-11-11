// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact Released v1 bytes before activating the supported v2 profile row.
use super::{record, storage::NativeState};
use crate::durable_storage::protected::{
    exclusion::{ExclusionOpen, HeldFile},
    native_name::{NativeName, NativeSid},
    native_phase::NativePhase,
    record_file,
};
use crate::{ClaimError, native_claims::files::Budget};
use sugarsubstitute_contracts::execution::JobContext;
#[cfg(test)]
mod tests;

/// Persist/recheck the exact legacy predecessor through the original protected publication owner.
/// Existing foreign backup bytes reject; partial outputs stay retained for checked shutdown.
pub(super) fn preserve(state: &mut NativeState, context: &JobContext) -> Result<(), ClaimError> {
    let Some(original) = state.predecessor else {
        return Ok(());
    };
    if original[8..10] != [1, 0] {
        return Ok(());
    }
    if record::Record::decode(&original)?.state != record::State::Released {
        return Err(ClaimError::invalid("profile_migration_legacy_active"));
    }
    let root = state.root.as_ref().ok_or_else(|| ClaimError::invalid("profile_migration_root"))?;
    let (folder, _, sid) = root.profile_folder()?;
    let budget = Budget::Protected(
        state
            .memory
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("profile_migration_memory"))?
            .clone(),
    );
    if state.migration_file.is_none() {
        HeldFile::acquire_into(
            ExclusionOpen::CreateOrOpen,
            &mut state.migration_file,
            NativeName::Ordinary(folder.join(crate::profiles::locations::ORIGINAL_CLAIM)),
            NativeName::Ordinary(folder.join(crate::profiles::locations::ORIGINAL_RECORD)),
            NativeSid::Ordinary(sid.to_vec()),
            budget.clone(),
            &mut NativePhase::ordinary(&budget)?,
        )?;
    }
    let file = state
        .migration_file
        .as_ref()
        .ok_or_else(|| ClaimError::invalid("profile_migration_file"))?;
    match record_file::read_shape(file, context)? {
        record_file::RecordRead::Present(bytes) if bytes == original => {}
        record_file::RecordRead::Missing if file.created => record_file::publish(
            file,
            &original,
            record_file::PublicationAuthority::Ordinary,
            context,
            |old| match old {
                record_file::RecordRead::Missing => Ok(()),
                record_file::RecordRead::Present(bytes) if *bytes == original => Ok(()),
                _ => Err(ClaimError::invalid("profile_migration_changed_original")),
            },
        )?,
        _ => return Err(ClaimError::invalid("profile_migration_unknown_original")),
    }
    if let Some(file) = &mut state.migration_file {
        file.close()?;
    }
    state.migration_file = None;
    Ok(())
}
