// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate genuine protected Root opening custody from the lost handler registration.
use super::*;
use crate::incident::{CapturedReport, IncidentStoreError};
use crate::native::RootStorageParts as Parts;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_execution::RegisteredRecoveryGrant;
use sugarsubstitute_supervision::diagnostic_storage::*;
pub(crate) struct Owner {
    parts: Mutex<Option<Parts>>,
    borrowed: std::sync::atomic::AtomicBool,
}
/// Retain the same actual1MiB protected Root and source allocation independently of handler close.
/// Every alias retains its outside original native/source charge after its Root control.
/// No constructor accepts a path, handle number, token or externally decoded source facts.
#[derive(Clone)]
pub struct CaptureRootSource {
    pub(super) owner: Arc<Owner>,
    pub(super) source: Arc<Shared>,
}
impl WindowsCrashCapture {
    /// Transfer protected Root custody once after successful actual registration.
    /// This cache-only move performs no native call, source reconstruction or new claim.
    /// Handler cleanup can subsequently retire its own actual process/I/O normally.
    /// # Errors
    /// Rejects incomplete/foreign/recovery registration, repeated transfer or in-flight loan.
    pub fn retain_root_source(&self) -> Result<CaptureRootSource, CrashError> {
        let shared = self.shared.as_ref().ok_or_else(|| {
            CrashError::invalid(sugarsubstitute_contracts::crash::CaptureOperation::Observation)
        })?;
        {
            let state = lock(&shared.native);
            if !state.registered || state.retired {
                return Err(CrashError::invalid(
                    sugarsubstitute_contracts::crash::CaptureOperation::Observation,
                ));
            }
        }
        let mut slot = lock(&shared.root_source);
        if slot.strong_count() != 0 || slot.weak_count() != 0 {
            return Err(CrashError::invalid(
                sugarsubstitute_contracts::crash::CaptureOperation::Observation,
            ));
        }
        let mut loan = loan::NativeLoan::take(
            shared.clone(),
            sugarsubstitute_contracts::crash::CaptureOperation::Observation,
        )?
        .ok_or_else(|| {
            CrashError::invalid(sugarsubstitute_contracts::crash::CaptureOperation::Observation)
        })?;
        let parts = loan.lease()?.take_root_storage()?;
        let owner = Arc::new(Owner {
            parts: Mutex::new(Some(parts)),
            borrowed: std::sync::atomic::AtomicBool::new(false),
        });
        *slot = Arc::downgrade(&owner);
        drop(slot);
        Ok(CaptureRootSource { owner, source: shared.clone() })
    }
}
impl CaptureRootSource {
    /// Compare aliases of the identical retained native Root and original registration owner.
    /// This creates no file, process, capture or historical authority.
    pub fn same_source(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner) && Arc::ptr_eq(&self.source, &other.source)
    }

    /// Copy the actual registered producer build without substituting this process's package version.
    /// No file/native call or new registration authority is issued.
    pub fn registration_build(&self) -> Option<super::CaptureBuildIdentity> {
        *lock(&self.source.build)
    }
    pub(crate) fn recorded_build(
        &self,
    ) -> Result<sugarsubstitute_contracts::incident::BuildIdentity, IncidentStoreError> {
        let selected = self.registration_build().ok_or(IncidentStoreError::RecoveryConflict)?;
        let text = selected.as_str().map_err(|cause| {
            lock(&self.source.native).retain(cause);
            IncidentStoreError::RecoveryConflict
        })?;
        Ok(sugarsubstitute_contracts::incident::BuildIdentity::new(text)?)
    }
    /// Borrow actual retained canonical Root identity, never an opening capability.
    pub fn root_identity(&self) -> Option<[u8; 24]> {
        lock(&self.owner.parts).as_ref()?.storage.root_identity()
    }
    /// Construct the ordinary finished-source view from the same actual Root custody.
    pub fn finished_source(&self) -> FinishedCaptureSource {
        FinishedCaptureSource::from_root(self.clone())
    }
    pub(crate) fn origin_root(
        &self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<[u8; 24], IncidentStoreError> {
        if context.owner_id() != self.source.execution_id
            || operation.key().owner() != self.source.owner
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        self.root_identity().ok_or(IncidentStoreError::RecoveryConflict)
    }
    pub(crate) fn check_origin_read(
        &self,
        read: DiagnosticReadLease,
        validator: &impl RecoveryCaptureOriginValidator,
        storage: &mut DiagnosticStorage,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<CheckedRecoveryCaptureOrigin, IncidentStoreError> {
        self.origin_root(operation, context)?;
        let mut loan = StorageLoan::take(self.clone())?;
        let parts = loan.parts.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        Ok(storage.check_recovery_capture_origin(
            &parts.storage,
            read,
            validator,
            operation,
            context,
        )?)
    }
    /// Prepare a historical recovery capture only from a protected journal read under this current Root.
    /// Historical Root epochs remain matching data; this source/context supplies current authority.
    /// # Errors
    /// Wrong current operation, changed root, revoked read, native or deadline failure retains partials.
    pub fn prepare_historical_recovery_finished(
        &self,
        checked: &CheckedRecoveryCaptureOrigin,
        output: &mut Option<CapturedReport>,
        storage: &mut DiagnosticStorage,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        self.origin_root(operation, context)?;
        let mut loan = StorageLoan::take(self.clone())?;
        let parts = loan.parts.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        if output.is_none() {
            *output = Some(CapturedReport::new(self.source.clone()));
        }
        let report = output.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        if !Arc::ptr_eq(&report.source, &self.source) || report.token.is_some() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if report.generation.is_none() {
            storage.prepare_historical_recovery_capture_read(
                &mut parts.storage,
                checked,
                &mut report.generation,
                operation,
                context,
            )?;
            report.recovery_origin = true;
        } else if report
            .generation
            .as_ref()
            .ok_or(IncidentStoreError::RecoveryConflict)?
            .origin()?
            != sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Recovery(checked.origin())
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if report.cpp.is_none() && report.rust.is_none() {
            let (cpp, rust) = storage.reserve_finished_owner(
                report.generation.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?,
                operation,
                context,
            )?;
            report.cpp = Some(cpp);
            report.rust = Some(rust);
        }
        parts.install(report, context, &self.source.execution)
    }
    /// Prepare an unused local native owner for one actual canonical promotion replay.
    /// The strict validator and actual protected Journal remain bound to this current Root.
    /// # Errors
    /// Changed source/full origin or partial native output preserves the original issued owner.
    pub fn prepare_promoted_finished(
        &self,
        journal: &mut DiagnosticReadLease,
        validator: &impl CapturePromotionValidator,
        output: &mut Option<CapturedReport>,
        storage: &mut DiagnosticStorage,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        self.origin_root(operation, context)?;
        let mut loan = StorageLoan::take(self.clone())?;
        let parts = loan.parts.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        if output.is_none() {
            *output = Some(CapturedReport::new(self.source.clone()));
        }
        let report = output.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        if !Arc::ptr_eq(&report.source, &self.source) || report.token.is_some() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        storage.prepare_capture_promotion_read(
            &mut parts.storage,
            journal,
            validator,
            &mut report.generation,
            operation,
            context,
        )?;
        report.recovery_origin = true;
        let (cpp, rust) = storage.reserve_finished_owner(
            report.generation.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?,
            operation,
            context,
        )?;
        report.cpp = Some(cpp);
        report.rust = Some(rust);
        parts.install(report, context, &self.source.execution)
    }
    /// Prepare an exact granted generation using a new token pinned in this Root process.
    /// # Errors
    /// Foreign/current grant, partial source, native/queue/cancellation errors retain all owners.
    pub fn prepare_recovery_finished(
        &self,
        grant: &RegisteredRecoveryGrant,
        output: &mut Option<CapturedReport>,
        storage: &mut DiagnosticStorage,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        self.prepare(Some(grant), output, storage, operation, context)
    }
    pub(super) fn prepare(
        &self,
        grant: Option<&RegisteredRecoveryGrant>,
        output: &mut Option<CapturedReport>,
        storage: &mut DiagnosticStorage,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        if context.owner_id() != self.source.execution_id
            || operation.key().owner() != self.source.owner
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let mut loan = StorageLoan::take(self.clone())?;
        let parts = loan.parts.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        if output.is_none() {
            *output = Some(CapturedReport::new(self.source.clone()));
        }
        let report = output.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        if !Arc::ptr_eq(&report.source, &self.source) || report.token.is_some() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if report.generation.is_none() {
            if let Some(grant) = grant {
                storage.prepare_recovery_capture_read(
                    &mut parts.storage,
                    grant,
                    &mut report.generation,
                    operation,
                    context,
                )?;
                report.recovery_origin = true;
            } else {
                storage.prepare_capture_read(
                    &mut parts.storage,
                    &mut report.generation,
                    operation,
                    context,
                )?;
            }
        }
        if report.cpp.is_none() && report.rust.is_none() {
            let (cpp, rust) = storage.reserve_finished_owner(
                report.generation.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?,
                operation,
                context,
            )?;
            report.cpp = Some(cpp);
            report.rust = Some(rust);
        }
        parts.install(report, context, &self.source.execution)
    }
    /// Checked-close this independent source after all actual recovery read/token custody retires.
    /// The first original15s cleanup context remains latched by the protected Root owner.
    /// # Errors
    /// Busy readers, changed/renewed context or failed close preserve the same complete custody.
    pub fn retire(&self, context: &JobContext) -> Result<(), CrashError> {
        if context.owner_id() != self.source.execution_id {
            return Err(CrashError::invalid(
                sugarsubstitute_contracts::crash::CaptureOperation::Retirement,
            ));
        }
        let mut loan = StorageLoan::take(self.clone()).map_err(|cause| {
            CrashError::new(sugarsubstitute_contracts::crash::CaptureOperation::Retirement, cause)
        })?;
        if let Some(parts) = loan.parts.as_mut() {
            parts.storage.retire(context).map_err(|cause| {
                CrashError::new(
                    sugarsubstitute_contracts::crash::CaptureOperation::Retirement,
                    cause,
                )
            })?;
        }
        Ok(())
    }
}
struct StorageLoan {
    original: CaptureRootSource,
    parts: Option<Parts>,
}
impl StorageLoan {
    fn take(original: CaptureRootSource) -> Result<Self, IncidentStoreError> {
        use std::sync::atomic::Ordering;
        if original
            .owner
            .borrowed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let parts = lock(&original.owner.parts).take();
        Ok(Self { original, parts })
    }
}
impl Drop for StorageLoan {
    fn drop(&mut self) {
        *lock(&self.original.owner.parts) = self.parts.take();
        self.original.owner.borrowed.store(false, std::sync::atomic::Ordering::Release);
    }
}
