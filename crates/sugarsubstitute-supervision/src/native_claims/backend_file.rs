// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acquire and publish the exact backend exclusion under its native claim owner.
use super::files::Budget;
use super::*;
/// Preserve backend exclusion custody through predecessor validation and publication.
/// # Errors
/// Retains adopted file outputs when protection, record or original context fails.
pub(crate) fn acquire_backend(
    backend: &mut Backend,
    directory: &Path,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), ClaimError> {
    if let Some(parent) = &backend.custodian {
        if parent.key().owner() != backend.resource.owner {
            return Err(ClaimError::invalid("backend_custodian_owner"));
        }
        parent.check().map_err(|cause| ClaimError::new("backend_custodian_current", cause))?;
    }
    let digest = crate::protected_storage::record_file::hex(&backend.proof.digest);
    let stem = directory.join("backends").join(digest);
    HeldFile::acquire_into(
        crate::protected_storage::exclusion::ExclusionOpen::CreateOrOpen,
        &mut backend.file,
        crate::protected_storage::native_name::NativeName::Ordinary(stem.with_extension("claim")),
        crate::protected_storage::native_name::NativeName::Ordinary(stem.with_extension("record")),
        crate::protected_storage::native_name::NativeSid::Ordinary(
            backend.proof.account_sid.clone(),
        ),
        Budget::Slot(backend.proof.slot.clone()),
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&Budget::Slot(
            backend.proof.slot.clone(),
        ))?,
    )?;
    // A surviving witness still owns the backend Job. Both locks must be held
    // before a successor can recover a witnessed generation after owner loss.
    HeldFile::acquire_into(
        crate::protected_storage::exclusion::ExclusionOpen::CreateOrOpen,
        &mut backend.custody_probe,
        crate::protected_storage::native_name::NativeName::Ordinary(stem.with_extension("custody")),
        crate::protected_storage::native_name::NativeName::Ordinary(stem.with_extension("record")),
        crate::protected_storage::native_name::NativeSid::Ordinary(
            backend.proof.account_sid.clone(),
        ),
        Budget::Slot(backend.proof.slot.clone()),
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&Budget::Slot(
            backend.proof.slot.clone(),
        ))?,
    )?;
    let file = backend.file.as_ref().ok_or_else(|| ClaimError::invalid("missing_claim"))?;
    let mut record = Record::backend(backend.resource, backend.proof.digest);
    if let Some(parent) = &backend.custodian {
        record.version = 2;
        record.custodian = parent.key().session().get();
    }
    backend.predecessor = backend_recovery::predecessor(
        file,
        backend
            .custody_probe
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("missing_custody_probe"))?,
        &backend.proof.account_sid,
        &record,
        context,
    )?;
    backend.record = Some(record.clone());
    backend_recovery::publish(file, &record, backend.predecessor.as_ref(), context)?;
    backend.claimed = true;
    backend
        .custody_probe
        .as_mut()
        .ok_or_else(|| ClaimError::invalid("missing_custody_probe"))?
        .close()?;
    backend.custody_probe = None;
    Ok(())
}
