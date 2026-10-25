// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Initialize coordinator claims through the sole actual account/namespace resolver.
use super::*;
/// Initialize the new generation while retaining election and coordinator custody.
/// Recovered initialization moves the original held coordinator file without reacquisition.
/// # Errors
/// Preserves native/account failures and rejects changed predecessors before publication.
pub(crate) fn initialize(
    root: &mut Root,
    installation: &Path,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), ClaimError> {
    let pool = root.pool.as_ref().ok_or_else(|| ClaimError::invalid("missing_pool"))?.clone();
    let budget = files::Budget::Core(pool);
    let resolved = crate::durable_storage::protected::account::resolve_namespace(
        installation,
        &mut root.handles,
        budget.clone(),
    )?;
    let mut guard = root
        .entry_election
        .as_ref()
        .map(|proof| proof.lock_for_initialization(context, &resolved))
        .transpose()?;
    let namespace = resolved.namespace;
    root.sid = resolved.account_sid;
    root.directory = resolved.directory;
    let directory = &root.directory;
    let epoch = sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new(
        crate::durable_storage::protected::record_file::random(
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(&budget)?,
        )?,
    )
    .map_err(|e| ClaimError::new("coordinator_epoch", e))?;
    let key = CoordinatorKey { namespace, epoch };
    root.key = Some(key);
    let stem = directory
        .join("coordinators")
        .join(crate::durable_storage::protected::record_file::hex(namespace.bytes()));
    if let Some(guard) = guard.as_mut()
        && matches!(
            guard.proof.origin,
            crate::coordinator_entry::election::ElectionRecordOrigin::RecoveredHeld { .. }
        )
    {
        guard.adopt_recovered_coordinator(&mut root.coordinator)?;
    } else {
        HeldFile::acquire_into(
            crate::durable_storage::protected::exclusion::ExclusionOpen::CreateOrOpen,
            &mut root.coordinator,
            crate::durable_storage::protected::native_name::NativeName::Ordinary(
                stem.with_extension("claim"),
            ),
            crate::durable_storage::protected::native_name::NativeName::Ordinary(
                stem.with_extension("record"),
            ),
            crate::durable_storage::protected::native_name::NativeSid::Ordinary(root.sid.clone()),
            budget.clone(),
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(&budget)?,
        )?;
    }
    let held = root.coordinator.as_ref().ok_or_else(|| ClaimError::invalid("missing_claim"))?;
    let record = Record::coordinator(key);
    if let Some(guard) = &guard {
        guard.check_record(&crate::durable_storage::protected::record_file::read_shape(
            held, context,
        )?)?;
    }
    if !guard.as_ref().is_some_and(|guard| {
        matches!(
            guard.proof.origin,
            crate::coordinator_entry::election::ElectionRecordOrigin::Fresh(_)
                | crate::coordinator_entry::election::ElectionRecordOrigin::RecoveredHeld { .. }
        )
    }) {
        root.original_released = records::check_predecessor(held, &root.sid, &record, context)?;
    }
    root.record = Some(record);
    let record = root.record.as_ref().ok_or_else(|| ClaimError::invalid("missing_record"))?;
    if let Some(guard) = &guard
        && matches!(
            guard.proof.origin,
            crate::coordinator_entry::election::ElectionRecordOrigin::Fresh(_)
                | crate::coordinator_entry::election::ElectionRecordOrigin::RecoveredHeld { .. }
        )
    {
        let permission = guard.publication(
            held,
            crate::durable_storage::protected::paths::file_identity(
                &held.handle,
                &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                    &held.handle.budget(),
                )?,
            )?,
        )?;
        records::publish_elected(held, record, &permission, context)?;
    } else {
        records::publish(held, record, context)?;
    }
    root.claimed = true;
    Ok(())
}
