// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain independent profile exclusion and exact prepared desktop Job in one witness slot.
use super::pool::WitnessSlot;
use crate::{
    ClaimError,
    native_claims::files::{Budget, Handle},
    native_process::{Identity, custody::CustodyFamily},
    profiles::record::custody::CustodiedRecord,
    protected_storage::{
        exclusion::{ExclusionOpen, HeldFile},
        native_name::{NativeName, NativeNameDestination, NativeSid},
        native_phase::NativePhase,
        paths, record_file, security,
    },
};
use std::{
    os::windows::io::{IntoRawHandle, OwnedHandle},
    path::PathBuf,
};
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyCommand as Command, BackendCustodyKey, ProfileCustodyKey},
    execution::{JobContext, Moment},
};
use sugarsubstitute_execution::backend_custody::BackendCustodyWake;

/// Every actual directory/file/family output precedes its same occupied slot guard.
pub(super) struct ProfileRegistration {
    family: CustodyFamily,
    directory: Option<Handle>,
    path: Option<PathBuf>,
    custody: Option<HeldFile<160>>,
    writer: Option<HeldFile<160>>,
    original: CustodiedRecord,
    key: ProfileCustodyKey,
    claim_identity: [u8; 24],
    sid: Vec<u8>,
    until: Moment,
    root_adopted: bool,
    recovery_grant: Option<sugarsubstitute_contracts::recovery_supervisor::RecoveryGrantId>,
    capture_wait: bool,
    recovery_settle_until: Option<Moment>,
    publication_acknowledged: bool,
    cleaning: bool,
    settled: bool,
    failed: bool,
    slot: WitnessSlot,
}
impl ProfileRegistration {
    /// Retire physical family before validating/publishing the exact profile row.
    /// Unknown or foreign predecessor bytes remain preserved and observable.
    pub(super) fn retire(
        &mut self,
        context: &JobContext,
        wake: &BackendCustodyWake,
    ) -> Result<bool, ClaimError> {
        if self.failed {
            return Ok(false);
        }
        let result = self.retire_owned(context, wake);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn retire_owned(
        &mut self,
        context: &JobContext,
        wake: &BackendCustodyWake,
    ) -> Result<bool, ClaimError> {
        if self.settled {
            return Ok(true);
        }
        if self.capture_wait {
            return Ok(false);
        }
        let proof = self.slot.cleanup_proof()?;
        proof.check().map_err(|cause| ClaimError::new("profile_custody_cleanup", cause))?;
        if self.writer.is_none() {
            let path =
                self.path.as_ref().ok_or_else(|| ClaimError::invalid("profile_custody_path"))?;
            let budget = Budget::Custody(self.slot.clone());
            let result = HeldFile::acquire_into(
                ExclusionOpen::Existing(self.claim_identity),
                &mut self.writer,
                NativeName::Ordinary(path.join(".sugarsubstitute-profile-v1.claim")),
                NativeName::Ordinary(path.join(".sugarsubstitute-profile-v1.record")),
                NativeSid::Ordinary(self.sid.clone()),
                budget.clone(),
                &mut NativePhase::ordinary(&budget)?,
            );
            if let Err(cause) = result {
                use std::error::Error;
                if cause.operation() == "claim_exclusion"
                    && cause.cleanup_cause().is_none()
                    && cause
                        .source()
                        .and_then(|source| source.downcast_ref::<std::fs::TryLockError>())
                        .is_some_and(|cause| matches!(cause, std::fs::TryLockError::WouldBlock))
                {
                    if let Some(writer) = &mut self.writer {
                        writer.close()?;
                    }
                    self.writer = None;
                    proof
                        .observe_contention()
                        .map_err(|cause| ClaimError::new("profile_custody_contention", cause))?;
                    return Ok(false);
                }
                return Err(cause);
            }
        }
        if !self
            .family
            .retire(wake)
            .map_err(|cause| ClaimError::new("profile_custody_complete_family", cause))?
        {
            return Ok(false);
        }
        let writer =
            self.writer.as_ref().ok_or_else(|| ClaimError::invalid("profile_custody_writer"))?;
        match record_file::read_shape(writer, context)? {
            record_file::RecordRead::Present(bytes) => {
                if bytes == self.original.encode() || bytes == self.original.released().encode() {
                    // The prospective row includes this actual helper session and
                    // prepared family; atomic byte match proves publication even
                    // when parent loss prevents its Published confirmation.
                    let released = self.original.released().encode();
                    record_file::retire_publication(writer, Some(&bytes), context)?;
                    if bytes != released {
                        record_file::publish(
                            writer,
                            &released,
                            record_file::PublicationAuthority::Ordinary,
                            context,
                            |old| match old {
                                record_file::RecordRead::Present(actual)
                                    if *actual == self.original.encode() =>
                                {
                                    Ok(())
                                }
                                _ => {
                                    Err(ClaimError::invalid("profile_custody_changed_publication"))
                                }
                            },
                        )?;
                    }
                } else if self.root_adopted {
                    return Err(ClaimError::invalid("profile_custody_changed_record"));
                }
                // Before root adoption, publication may never have been issued;
                // predecessor bytes have no release authority and stay unchanged.
            }
            record_file::RecordRead::Missing | record_file::RecordRead::Empty
                if !self.root_adopted => {}
            _ => return Err(ClaimError::invalid("profile_custody_record_missing")),
        }
        if let Some(writer) = &mut self.writer {
            writer.close()?;
        }
        self.writer = None;
        if let Some(custody) = &mut self.custody {
            custody.close()?;
        }
        self.custody = None;
        if let Some(directory) = &mut self.directory {
            directory.close()?;
        }
        self.directory = None;
        self.slot.retired();
        self.settled = true;
        Ok(true)
    }
    /// Validate prospective bytes before adopting actual prepared outputs.
    pub(super) fn new(
        slot: WitnessSlot,
        sid: &[u8],
        parent: BackendCustodyKey,
        command: &Command,
        until: Moment,
    ) -> Result<Self, ClaimError> {
        let Command::ProfilePrepare { profile, recovery_grant, record, claim_identity, .. } =
            command
        else {
            return Err(ClaimError::invalid("profile_custody_prepare_command"));
        };
        let original = CustodiedRecord::decode(record)?;
        if !original.matches_preparation(parent, *profile) || !original.matches_account(sid) {
            return Err(ClaimError::invalid("profile_custody_prepare_record"));
        }
        slot.prepare(until)?;
        let family = CustodyFamily::new(&slot)
            .map_err(|cause| ClaimError::new("profile_custody_family", cause))?;
        Ok(Self {
            family,
            directory: None,
            path: None,
            custody: None,
            writer: None,
            original,
            key: *profile,
            claim_identity: *claim_identity,
            sid: sid.to_vec(),
            until,
            root_adopted: false,
            recovery_grant: *recovery_grant,
            capture_wait: false,
            recovery_settle_until: None,
            publication_acknowledged: false,
            cleaning: false,
            settled: false,
            failed: false,
            slot,
        })
    }
    /// Adopt both original transferred outputs before path/security/native validation.
    pub(super) fn prepare(
        &mut self,
        job: OwnedHandle,
        directory: OwnedHandle,
    ) -> Result<(), ClaimError> {
        self.directory = Some(Handle::new(
            windows::Win32::Foundation::HANDLE(directory.into_raw_handle()),
            Budget::Custody(self.slot.clone()),
        ));
        self.family
            .adopt_job(job)
            .map_err(|cause| ClaimError::new("profile_custody_prepared_job", cause))?;
        let handle = self
            .directory
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("profile_custody_directory"))?;
        let budget = Budget::Custody(self.slot.clone());
        let mut phase = NativePhase::ordinary(&budget)?;
        paths::validate_object(handle, true, true, &mut phase)?;
        if paths::file_identity(handle, &mut phase)? != self.original.folder() {
            return Err(ClaimError::invalid("profile_custody_folder_identity"));
        }
        security::verify(handle, &self.sid, &mut phase)?;
        self.path = Some(
            paths::final_path(handle, &mut phase, NativeNameDestination::Ordinary)?
                .into_ordinary()?,
        );
        let path = self.path.as_ref().ok_or_else(|| ClaimError::invalid("profile_custody_path"))?;
        HeldFile::acquire_into(
            ExclusionOpen::CreateOrOpen,
            &mut self.custody,
            NativeName::Ordinary(path.join(".sugarsubstitute-profile-v2.custody")),
            NativeName::Ordinary(path.join(".sugarsubstitute-profile-v1.record")),
            NativeSid::Ordinary(self.sid.clone()),
            budget.clone(),
            &mut NativePhase::ordinary(&budget)?,
        )
    }
    pub(super) const fn key(&self) -> ProfileCustodyKey {
        self.key
    }
    /// Acknowledge only exact actual Held activation after its original Prepared receipt.
    pub(super) fn published(
        &mut self,
        profile: ProfileCustodyKey,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        if profile != self.key || self.publication_acknowledged || self.root_adopted {
            return Err(ClaimError::invalid("profile_custody_publication_phase"));
        }
        let custody = self
            .custody
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("profile_custody_exclusion"))?;
        match record_file::read_shape(custody, context)? {
            record_file::RecordRead::Present(bytes) if bytes == self.original.encode() => {}
            _ => return Err(ClaimError::invalid("profile_custody_publication_mismatch")),
        }
        self.publication_acknowledged = true;
        Ok(())
    }
    pub(super) const fn original_until(&self) -> Moment {
        self.until
    }
    pub(super) const fn cleaning(&self) -> bool {
        self.cleaning
    }
    pub(super) const fn settled(&self) -> bool {
        self.settled
    }
    pub(super) fn root_signaled(&self) -> bool {
        self.family.root_signaled()
    }
    /// Keep the first original cleanup proof for all subsequent native retirement.
    pub(super) fn stop(&mut self, until: Moment) -> Result<(), ClaimError> {
        if !self.cleaning {
            self.slot.cleanup(until)?;
            self.cleaning = true;
        }
        Ok(())
    }
    /// Match actual suspended-root identity to the same prepared profile/family.
    pub(super) fn root(
        &mut self,
        command: &Command,
        input: OwnedHandle,
        wake: &BackendCustodyWake,
    ) -> Result<(), ClaimError> {
        let Command::ProfileRoot { profile, pid, birth, .. } = command else {
            return Err(ClaimError::invalid("profile_custody_root_command"));
        };
        if *profile != self.key || self.root_adopted || !self.publication_acknowledged {
            return Err(ClaimError::invalid("profile_custody_root_association"));
        }
        self.family
            .adopt_root(input, Identity { pid: *pid, birth: *birth }, wake)
            .map_err(|cause| ClaimError::new("profile_custody_suspended_root", cause))?;
        self.slot.root_adopted()?;
        self.root_adopted = true;
        Ok(())
    }
}

mod loss;
