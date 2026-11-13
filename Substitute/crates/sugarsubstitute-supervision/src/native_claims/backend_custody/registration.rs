// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle only the authenticated prepared Job under its independent native exclusion.
use super::pool::WitnessSlot;
use crate::{
    ClaimError,
    durable_storage::protected::{
        exclusion::{ExclusionOpen, HeldFile},
        native_name::{NativeName, NativeSid},
        native_phase::NativePhase,
        record_file,
    },
    native_claims::{Record, RecordState, files::Budget, records},
    native_process::{Identity, custody::CustodyFamily},
};
use std::{
    error::Error,
    os::windows::io::OwnedHandle,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyCommand, BackendCustodyKey},
    execution::{JobContext, Moment},
    lifecycle::ResourceKey,
};
use sugarsubstitute_execution::backend_custody::BackendCustodyWake;

/// Keep each output and stored cause before the original occupied slot guard.
pub(super) struct Registration {
    family: CustodyFamily,
    custody: Option<HeldFile>,
    claim: Option<HeldFile>,
    stem: PathBuf,
    sid: Vec<u8>,
    original: Record,
    resource_key: ResourceKey,
    claim_identity: [u8; 24],
    registry_process: u64,
    original_until: Moment,
    root_adopted: bool,
    contention: bool,
    settled: bool,
    cleaning: bool,
    failed: bool,
    slot: WitnessSlot,
}
impl Registration {
    /// Allocate funded empty cells; the driver installs this owner before adoption.
    pub(super) fn new(
        slot: WitnessSlot,
        directory: &Path,
        sid: &[u8],
        key: BackendCustodyKey,
        command: &BackendCustodyCommand,
        until: Moment,
    ) -> Result<Self, ClaimError> {
        let BackendCustodyCommand::Prepare {
            resource,
            digest,
            record,
            claim_identity,
            registry_process,
            ..
        } = command
        else {
            return Err(ClaimError::invalid("custody_prepare_command"));
        };
        let original = Record::decode(record)?;
        if original.version != 2
            || original.custodian != key.session().get()
            || original.owner != key.owner()
            || original.owner != resource.owner
            || original.resource != resource.id.get()
            || original.generation != resource.generation.get()
            || original.digest != *digest
            || original.process != 0
            || !matches!(original.state, RecordState::Held | RecordState::LaunchPending)
        {
            return Err(ClaimError::invalid("custody_prepare_record"));
        }
        slot.prepare(until)?;
        let family = CustodyFamily::new(&slot)
            .map_err(|cause| ClaimError::new("custody_family_storage", cause))?;
        Ok(Self {
            family,
            custody: None,
            claim: None,
            stem: directory.join("backends").join(record_file::hex(digest)),
            sid: sid.to_vec(),
            original,
            resource_key: *resource,
            claim_identity: *claim_identity,
            registry_process: *registry_process,
            original_until: until,
            root_adopted: false,
            contention: false,
            settled: false,
            cleaning: false,
            failed: false,
            slot,
        })
    }
    /// Retain the actual Job output before validating it or opening exclusion.
    pub(super) fn prepare(&mut self, input: OwnedHandle) -> Result<(), ClaimError> {
        self.family
            .adopt_job(input)
            .map_err(|cause| ClaimError::new("custody_prepared_job", cause))?;
        let budget = Budget::Custody(self.slot.clone());
        HeldFile::acquire_into(
            ExclusionOpen::CreateOrOpen,
            &mut self.custody,
            NativeName::Ordinary(self.stem.with_extension("custody")),
            NativeName::Ordinary(self.stem.with_extension("record")),
            NativeSid::Ordinary(self.sid.clone()),
            budget.clone(),
            &mut NativePhase::ordinary(&budget)?,
        )
    }
    /// Adopt the same suspended root before returning its correlated acknowledgement.
    pub(super) fn root(
        &mut self,
        command: &BackendCustodyCommand,
        input: OwnedHandle,
        wake: &BackendCustodyWake,
    ) -> Result<(), ClaimError> {
        self.validate_root(command)?;
        let BackendCustodyCommand::Root { pid, birth, .. } = command else {
            return Err(ClaimError::invalid("custody_root_command"));
        };
        self.family
            .adopt_root(input, Identity { pid: *pid, birth: *birth }, wake)
            .map_err(|cause| ClaimError::new("custody_suspended_root", cause))?;
        self.slot.root_adopted()?;
        self.root_adopted = true;
        Ok(())
    }
    /// Validate immutable root facts before moving its destination handle.
    pub(super) fn validate_root(&self, command: &BackendCustodyCommand) -> Result<(), ClaimError> {
        let BackendCustodyCommand::Root { resource, registry_process, .. } = command else {
            return Err(ClaimError::invalid("custody_root_command"));
        };
        if self.resource() != *resource
            || self.registry_process != *registry_process
            || self.root_adopted
        {
            return Err(ClaimError::invalid("custody_root_association"));
        }
        Ok(())
    }
    /// Return the unchanged original preparation cutoff.
    pub(super) const fn original_until(&self) -> Moment {
        self.original_until
    }
    /// Return only the first latched actual cleanup cutoff.
    pub(super) fn cleanup_until(&self) -> Result<Moment, ClaimError> {
        Ok(self.slot.cleanup_proof()?.until())
    }
    /// Return exact original resource matching facts without native authority.
    pub(super) fn resource(&self) -> ResourceKey {
        self.resource_key
    }
    /// Observe only an actual retained root wait, never PID/name absence.
    pub(super) fn root_signaled(&self) -> bool {
        self.family.root_signaled()
    }
    /// Start the first original cleanup; every later request keeps this cutoff.
    pub(super) fn stop(&mut self, until: Moment) -> Result<(), ClaimError> {
        if self.cleaning {
            return Ok(());
        }
        self.slot.cleanup(until)?;
        self.cleaning = true;
        Ok(())
    }
    pub(super) const fn cleaning(&self) -> bool {
        self.cleaning
    }
    pub(super) const fn settled(&self) -> bool {
        self.settled
    }
    /// Recover exclusion nonblockingly and settle complete native custody.
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
    /// Retain the first uncertain native failure rather than retrying its effects.
    fn retire_owned(
        &mut self,
        context: &JobContext,
        wake: &BackendCustodyWake,
    ) -> Result<bool, ClaimError> {
        if self.settled {
            return Ok(true);
        }
        let proof = self.slot.cleanup_proof()?;
        proof.check().map_err(|cause| ClaimError::new("custody_retirement_control", cause))?;
        if self.contention && !proof.contention_observed() {
            return Ok(false);
        }
        if self.claim.is_none() {
            let budget = Budget::Custody(self.slot.clone());
            let result = HeldFile::acquire_into(
                ExclusionOpen::Existing(self.claim_identity),
                &mut self.claim,
                NativeName::Ordinary(self.stem.with_extension("claim")),
                NativeName::Ordinary(self.stem.with_extension("record")),
                NativeSid::Ordinary(self.sid.clone()),
                budget.clone(),
                &mut NativePhase::ordinary(&budget)?,
            );
            if let Err(cause) = result {
                if is_contention(&cause) {
                    if let Some(file) = &mut self.claim {
                        file.close()?;
                    }
                    self.claim = None;
                    self.contention = true;
                    proof
                        .observe_contention()
                        .map_err(|cause| ClaimError::new("custody_lock_observation", cause))?;
                    return Ok(false);
                }
                return Err(cause);
            }
            self.contention = false;
        }
        if !self
            .family
            .retire(wake)
            .map_err(|cause| ClaimError::new("custody_complete_family", cause))?
        {
            return Ok(false);
        }
        let file =
            self.claim.as_ref().ok_or_else(|| ClaimError::invalid("custody_claim_missing"))?;
        let actual = match record_file::read_shape(file, context)? {
            record_file::RecordRead::Present(bytes) => Record::decode(&bytes)?,
            _ => return Err(ClaimError::invalid("custody_record_missing")),
        };
        self.validate_record(&actual)?;
        let mut expected = Some(actual);
        records::retire_backend(file, &mut expected, None, context)?;
        if let Some(file) = &mut self.claim {
            file.close()?;
        }
        self.claim = None;
        if let Some(file) = &mut self.custody {
            file.close()?;
        }
        self.custody = None;
        self.slot.retired();
        self.settled = true;
        Ok(true)
    }
    /// Reject legacy unresolved/foreign bytes before any Released publication.
    fn validate_record(&self, actual: &Record) -> Result<(), ClaimError> {
        if actual.version != 2
            || actual.owner != self.original.owner
            || actual.resource != self.original.resource
            || actual.generation != self.original.generation
            || actual.digest != self.original.digest
            || (actual.state == RecordState::Owned && actual.process != self.registry_process)
        {
            return Err(ClaimError::invalid("custody_record_association"));
        }
        Ok(())
    }
}
/// Only genuine lock contention without a secondary control failure permits observation.
fn is_contention(cause: &ClaimError) -> bool {
    cause.operation() == "claim_exclusion"
        && cause.cleanup_cause().is_none()
        && cause
            .source()
            .and_then(|source| source.downcast_ref::<std::fs::TryLockError>())
            .is_some_and(|cause| matches!(cause, std::fs::TryLockError::WouldBlock))
}
