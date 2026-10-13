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
    let digest = crate::protected_storage::record_file::hex(&backend.proof.digest);
    let stem = directory.join("backends").join(digest);
    HeldFile::acquire_into(
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
    let file = backend.file.as_ref().ok_or_else(|| ClaimError::invalid("missing_claim"))?;
    let record = Record::backend(backend.resource, backend.proof.digest);
    backend.original_released =
        records::check_predecessor(file, &backend.proof.account_sid, &record, context)?;
    backend.record = Some(record.clone());
    records::publish(file, &record, context)?;
    backend.claimed = true;
    Ok(())
}
