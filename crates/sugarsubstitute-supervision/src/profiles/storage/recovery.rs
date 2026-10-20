// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover abandoned witnessed profiles under both independent native exclusions.
use super::*;
use crate::protected_storage::{
    exclusion::ExclusionOpen,
    native_name::{NativeName, NativeSid},
    native_phase::NativePhase,
};

/// Retain partial recovery outputs until the original finite job retires them.
#[derive(Default)]
pub(super) struct Recovery {
    custody: Option<HeldFile<160>>,
    backup: Option<HeldFile<160>>,
    original: Option<[u8; 160]>,
    admitted: bool,
}
impl Recovery {
    /// Distinguish an observed abandoned witness from an ordinary unlocked writer.
    pub(super) fn admitted(&self) -> bool {
        self.admitted
    }

    /// Preserve the Held row only after both its writer and witness locks are acquired.
    /// The two owners retain the kill-on-close desktop Job; a surviving witness prevents recovery.
    /// The caller keeps the writer locked through publication and new witness admission.
    pub(super) fn recover(
        &mut self,
        writer: &HeldFile<160>,
        folder: &Path,
        sid: &[u8],
        original: &[u8; 160],
        budget: Budget,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        check(context)?;
        let old = record::custody::CustodiedRecord::decode(original)?;
        if !writer.is_locked() || old.original.state != record::State::Held {
            return Err(ClaimError::invalid("profile_recovery_writer"));
        }
        HeldFile::acquire_into(
            ExclusionOpen::CreateOrOpen,
            &mut self.custody,
            NativeName::Ordinary(folder.join(".sugarsubstitute-profile-v2.custody")),
            NativeName::Ordinary(folder.join(".sugarsubstitute-profile-v1.record")),
            NativeSid::Ordinary(sid.to_vec()),
            budget.clone(),
            &mut NativePhase::ordinary(&budget)?,
        )?;
        if !matches!(record_file::read_shape(writer, context)?,
            record_file::RecordRead::Present(bytes) if bytes == *original)
        {
            return Err(ClaimError::invalid("profile_recovery_changed_predecessor"));
        }
        let epoch = old
            .original
            .run
            .owner
            .epoch
            .bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let leaf = format!(
            ".sugarsubstitute-profile-recovered-{epoch}-{}-{}",
            old.original.run.instance.get(),
            old.original.run.generation.get()
        );
        self.original = Some(*original);
        HeldFile::acquire_into(
            ExclusionOpen::CreateOrOpen,
            &mut self.backup,
            NativeName::Ordinary(folder.join(format!("{leaf}.claim"))),
            NativeName::Ordinary(folder.join(format!("{leaf}.record"))),
            NativeSid::Ordinary(sid.to_vec()),
            budget.clone(),
            &mut NativePhase::ordinary(&budget)?,
        )?;
        let backup =
            self.backup.as_ref().ok_or_else(|| ClaimError::invalid("profile_recovery_backup"))?;
        match record_file::read_shape(backup, context)? {
            record_file::RecordRead::Present(bytes) if bytes == *original => {}
            record_file::RecordRead::Missing if backup.created => record_file::publish(
                backup,
                original,
                record_file::PublicationAuthority::Ordinary,
                context,
                |previous| match previous {
                    record_file::RecordRead::Missing => Ok(()),
                    _ => Err(ClaimError::invalid("profile_recovery_changed_backup")),
                },
            )?,
            _ => return Err(ClaimError::invalid("profile_recovery_unknown_backup")),
        }
        // Successful publication already settled its stage. Closing these guards
        // is ordinary setup, not a second transaction with a shutdown deadline.
        self.close()?;
        self.admitted = true;
        tracing::info!(operation="profile_abandoned_recovered", previous_run=?old.original.run,
            "Recovered abandoned profile after acquiring writer and witness exclusions");
        Ok(())
    }

    /// Settle partial backup publication before releasing either native output.
    pub(super) fn retire(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        if let Some(backup) = &mut self.backup {
            record_file::retire_publication(backup, self.original.as_ref(), context)?;
        }
        self.close()
    }

    fn close(&mut self) -> Result<(), ClaimError> {
        if let Some(backup) = &mut self.backup {
            backup.close()?;
        }
        self.backup = None;
        if let Some(custody) = &mut self.custody {
            custody.close()?;
        }
        self.custody = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
