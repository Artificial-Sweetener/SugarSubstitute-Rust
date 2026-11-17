// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover abandoned witnessed profiles under both independent native exclusions.
use super::*;
use crate::durable_storage::protected::{
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
            NativeName::Ordinary(folder.join(crate::profiles::locations::CUSTODY)),
            NativeName::Ordinary(folder.join(crate::profiles::locations::RECORD)),
            NativeSid::Ordinary(sid.to_vec()),
            budget.clone(),
            &mut NativePhase::ordinary(&budget)?,
        )?;
        if !matches!(record_file::read_shape(writer, context)?,
            record_file::RecordRead::Present(bytes) if bytes == *original)
        {
            return Err(ClaimError::invalid("profile_recovery_changed_predecessor"));
        }
        let leaf = crate::profiles::locations::recovered(old.original.run);
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

    /// Replace a record that names no earlier lifetime with a Released one for this store.
    ///
    /// Such a record is absent beside a claim that already existed, empty or zero bytes: what
    /// an interrupted first start, a disk fault or another program leaves. The caller holds the writer; this acquires the witness, which the helper keeping
    /// a desktop's kill-on-close Job alive holds for that desktop's whole life. Holding both
    /// proves no earlier desktop or browser lifetime remains. Stored bytes are moved beside the
    /// record with a fresh name and kept; the Released record lets the caller continue through
    /// its ordinary successor rules.
    /// # Errors
    /// A surviving witness refuses the exclusion, and the profile stays refused as before.
    pub(super) fn admit_nameless(
        &mut self,
        writer: &HeldFile<160>,
        folder: &Path,
        sid: &[u8],
        next: record::Record,
        budget: Budget,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        check(context)?;
        if !writer.is_locked() {
            return Err(ClaimError::invalid("profile_recovery_writer"));
        }
        HeldFile::acquire_into(
            ExclusionOpen::CreateOrOpen,
            &mut self.custody,
            NativeName::Ordinary(folder.join(crate::profiles::locations::CUSTODY)),
            NativeName::Ordinary(folder.join(crate::profiles::locations::RECORD)),
            NativeSid::Ordinary(sid.to_vec()),
            budget.clone(),
            &mut NativePhase::ordinary(&budget)?,
        )?;
        let stored = record_file::read_shape(writer, context)?;
        if !names_no_predecessor(&stored, writer.created) {
            return Err(ClaimError::invalid("profile_recovery_changed_predecessor"));
        }
        let aside = match stored {
            record_file::RecordRead::Missing => None,
            _ => Some(record_file::set_aside(writer, context)?),
        };
        let released = record::Record { state: record::State::Released, ..next }.encode();
        let witness =
            self.custody.as_ref().ok_or_else(|| ClaimError::invalid("profile_recovery_witness"))?;
        let published = record_file::publish(
            writer,
            &released,
            record_file::PublicationAuthority::Witnessed(witness),
            context,
            |previous| match previous {
                record_file::RecordRead::Missing => Ok(()),
                _ => Err(ClaimError::invalid("profile_recovery_changed_predecessor")),
            },
        );
        if published.is_err() {
            // A stage this publication left behind is settled before either exclusion closes.
            let settled =
                record_file::retire_publication(writer, Some(&released), context).map(|_| ());
            return ClaimError::finish(published, settled);
        }
        self.close()?;
        tracing::warn!(operation="profile_nameless_record_replaced", set_aside=?aside,
            "Replaced a profile record that named no earlier lifetime after acquiring writer and witness exclusions");
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

/// Decide whether what is stored where a profile record belongs names no earlier lifetime.
/// A record absent beside a claim this acquisition created is a first use, which names none
/// either but needs no recovery.
pub(super) fn names_no_predecessor(stored: &record_file::RecordRead<160>, created: bool) -> bool {
    match stored {
        record_file::RecordRead::Empty => true,
        record_file::RecordRead::Missing => !created,
        record_file::RecordRead::Present(bytes) => bytes.iter().all(|byte| *byte == 0),
    }
}

#[cfg(test)]
mod tests;
