// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Elect through both actual exclusions before adopting an exact stale Held predecessor.
use super::*;
/// Supply only filesystem outcomes; the election owner decides recovery under retained locks.
trait ElectionBoundary {
    /// Retain the entry exclusion; None reports contention without granting election.
    fn acquire_entry(&mut self) -> Result<Option<[u8; 24]>, ClaimError>;
    /// Read the original fixed record while preserving acquired entry custody.
    fn read_entry(&mut self) -> Result<RecordRead<128>, ClaimError>;
    /// Retain the coordinator exclusion while the entry exclusion stays held.
    fn acquire_coordinator(&mut self) -> Result<Option<[u8; 24]>, ClaimError>;
    /// Reread under both exclusions before permitting exact predecessor recovery.
    fn read_coordinator(&mut self) -> Result<RecordRead<128>, ClaimError>;
    /// Rename the unreadable record beside itself while both exclusions stay held.
    fn set_aside(&mut self) -> Result<std::path::PathBuf, ClaimError>;
    /// Release the coordinator exclusion so fresh initialization can acquire it again.
    fn release_coordinator(&mut self) -> Result<(), ClaimError>;
}
/// Borrow the actual native state and original job context throughout filesystem effects.
struct NativeBoundary<'a> {
    state: &'a mut NativeState,
    context: &'a JobContext,
}
impl NativeBoundary<'_> {
    /// Keep each acquired file in its original state slot through error and handoff.
    /// # Errors
    /// Preserves context, protected-file, security and file-identity failures.
    fn acquire(&mut self, entry: bool) -> Result<Option<[u8; 24]>, ClaimError> {
        check(self.context)?;
        let resolved = self
            .state
            .namespace
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("entry_namespace_missing"))?;
        let memory = self
            .state
            .memory
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("entry_memory_missing"))?
            .clone();
        let stem = resolved
            .directory
            .join("coordinators")
            .join(record_file::hex(resolved.namespace.bytes()));
        let slot =
            if entry { &mut self.state.election } else { &mut self.state.recovery_coordinator };
        if let Some(held) = slot.as_ref() {
            if !entry || !held.is_locked() {
                return Err(ClaimError::invalid("entry_repeated_exclusion"));
            }
            paths::validate_regular(
                &held.handle,
                Some(0),
                &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                    &held.handle.budget(),
                )?,
            )?;
            security::verify(
                &held.handle,
                &resolved.account_sid,
                &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                    &held.handle.budget(),
                )?,
            )?;
            return paths::file_identity(
                &held.handle,
                &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                    &held.handle.budget(),
                )?,
            )
            .map(Some);
        }
        let budget = Budget::Protected(memory);
        let acquired = HeldFile::acquire_into(
            crate::durable_storage::protected::exclusion::ExclusionOpen::CreateOrOpen,
            slot,
            crate::durable_storage::protected::native_name::NativeName::Ordinary(
                stem.with_extension(if entry { "entry.claim" } else { "claim" }),
            ),
            crate::durable_storage::protected::native_name::NativeName::Ordinary(
                stem.with_extension("record"),
            ),
            crate::durable_storage::protected::native_name::NativeSid::Ordinary(
                resolved.account_sid.clone(),
            ),
            budget.clone(),
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(&budget)?,
        );
        if let Err(cause) = acquired {
            return if is_contention(&cause) { Ok(None) } else { Err(cause) };
        }
        let held = slot.as_ref().ok_or_else(|| ClaimError::invalid("entry_exclusion_missing"))?;
        paths::file_identity(
            &held.handle,
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                &held.handle.budget(),
            )?,
        )
        .map(Some)
    }
}
impl ElectionBoundary for NativeBoundary<'_> {
    fn acquire_entry(&mut self) -> Result<Option<[u8; 24]>, ClaimError> {
        self.acquire(true)
    }
    fn read_entry(&mut self) -> Result<RecordRead<128>, ClaimError> {
        record_file::read_shape(
            self.state
                .election
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("entry_exclusion_missing"))?,
            self.context,
        )
    }
    fn acquire_coordinator(&mut self) -> Result<Option<[u8; 24]>, ClaimError> {
        self.acquire(false)
    }
    fn read_coordinator(&mut self) -> Result<RecordRead<128>, ClaimError> {
        record_file::read_shape(
            self.state
                .recovery_coordinator
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("entry_recovered_coordinator_missing"))?,
            self.context,
        )
    }
    fn set_aside(&mut self) -> Result<std::path::PathBuf, ClaimError> {
        record_file::set_aside(
            self.state
                .recovery_coordinator
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("entry_recovered_coordinator_missing"))?,
            self.context,
        )
    }
    fn release_coordinator(&mut self) -> Result<(), ClaimError> {
        let held = self
            .state
            .recovery_coordinator
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("entry_recovered_coordinator_missing"))?;
        check(self.context)?;
        held.close()?;
        self.state.recovery_coordinator = None;
        Ok(())
    }
}
/// Retain actual exclusions and original record facts for once-only claims initialization.
/// # Errors
/// Rejects repeated election and preserves uncertain native effects in their state slots.
pub(super) fn elect(state: &mut NativeState, context: &JobContext) -> Result<Elected, ClaimError> {
    check(context)?;
    if state.record_origin.is_some()
        || state.recovery_coordinator.as_ref().is_some_and(HeldFile::is_locked)
    {
        return Err(ClaimError::invalid("entry_election_repeated"));
    }
    for slot in [&mut state.recovery_coordinator, &mut state.election] {
        if let Some(held) = slot.as_mut()
            && !held.is_locked()
        {
            check(context)?;
            ClaimError::finish(held.close(), check(context))?;
            *slot = None;
        }
    }
    let namespace = state
        .namespace
        .as_ref()
        .ok_or_else(|| ClaimError::invalid("entry_namespace_missing"))?
        .namespace;
    let result = decide(&mut NativeBoundary { state, context }, namespace, context)?;
    match result {
        Elected::Fresh { origin, inode } => {
            state.entry_inode = Some(inode);
            state.record_origin = Some(ElectionRecordOrigin::Fresh(origin));
        }
        Elected::OrdinaryReleased { record, inode } => {
            state.entry_inode = Some(inode);
            state.record_origin = Some(ElectionRecordOrigin::Released(record));
        }
        Elected::RecoveredHeld { record, inode, coordinator_inode } => {
            state.entry_inode = Some(inode);
            state.record_origin =
                Some(ElectionRecordOrigin::RecoveredHeld { record, coordinator_inode });
        }
        _ => {}
    }
    Ok(result)
}
/// Classify protected records, then require both exclusions and an unchanged reread for Held recovery.
/// An unreadable record held under the entry exclusion is recovered only when the coordinator
/// exclusion also proves that no live owner remains.
/// # Errors
/// Preserves filesystem failures and rejects changed predecessors without granting authority.
fn decide(
    boundary: &mut dyn ElectionBoundary,
    namespace: sugarsubstitute_contracts::lifecycle::NamespaceId,
    context: &JobContext,
) -> Result<Elected, ClaimError> {
    check(context)?;
    let inode = ClaimError::finish(boundary.acquire_entry(), check(context))?;
    check(context)?;
    let read = match (ClaimError::finish(boundary.read_entry(), check(context)), inode) {
        (Err(cause), Some(inode)) if record_file::is_unreadable_length(&cause) => {
            return recover_unreadable(boundary, None, inode, context);
        }
        (read, _) => read?,
    };
    let original = match &read {
        RecordRead::Present(bytes) => Some(*bytes),
        _ => None,
    };
    let result = classify(read, namespace, inode)?;
    if let (Elected::Degraded(EntryOwnerDegradation::RecordUnknown), Some(inode), Some(_)) =
        (result, inode, original)
    {
        return recover_unreadable(boundary, original, inode, context);
    }
    if !matches!(result, Elected::Candidate(_)) || inode.is_none() {
        return Ok(result);
    }
    check(context)?;
    let coordinator_inode = ClaimError::finish(boundary.acquire_coordinator(), check(context))?;
    let Some(coordinator_inode) = coordinator_inode else { return Ok(result) };
    check(context)?;
    let actual = ClaimError::finish(boundary.read_coordinator(), check(context))?;
    let record =
        original.ok_or_else(|| ClaimError::invalid("entry_recovery_predecessor_missing"))?;
    if !matches!(actual,RecordRead::Present(bytes) if bytes==record) {
        return Err(ClaimError::invalid("entry_changed_recovery_predecessor"));
    }
    Ok(Elected::RecoveredHeld {
        record,
        inode: inode.ok_or_else(|| ClaimError::invalid("entry_recovery_inode_missing"))?,
        coordinator_inode,
    })
}
/// Move an undecodable, foreign or wrongly sized record aside once no owner can be alive.
///
/// `original` holds the full-length bytes first read, or `None` when the length itself was
/// unreadable. A busy coordinator exclusion means a live owner, which stays fail-closed as
/// `RecordUnknown`. Otherwise the reread must show the same unreadable record before its bytes
/// are preserved under a sibling name and the election proceeds from a missing record.
/// # Errors
/// Preserves filesystem failures and rejects a record that changed between reads; both
/// acquired exclusions then stay with the election owner.
fn recover_unreadable(
    boundary: &mut dyn ElectionBoundary,
    original: Option<[u8; 128]>,
    inode: [u8; 24],
    context: &JobContext,
) -> Result<Elected, ClaimError> {
    check(context)?;
    if ClaimError::finish(boundary.acquire_coordinator(), check(context))?.is_none() {
        return Ok(Elected::Degraded(EntryOwnerDegradation::RecordUnknown));
    }
    check(context)?;
    let unchanged = match (boundary.read_coordinator(), original) {
        (Ok(RecordRead::Present(actual)), Some(original)) => actual == original,
        (Err(cause), None) if record_file::is_unreadable_length(&cause) => true,
        (Err(cause), _) if !record_file::is_unreadable_length(&cause) => return Err(cause),
        _ => false,
    };
    check(context)?;
    if !unchanged {
        return Err(ClaimError::invalid("entry_changed_recovery_predecessor"));
    }
    let aside = ClaimError::finish(boundary.set_aside(), check(context))?;
    tracing::warn!(
        operation = "coordinator_entry_election",
        preserved = %aside.file_name().unwrap_or_default().to_string_lossy(),
        "Moved an unreadable coordinator record aside with no live owner and recovered the election"
    );
    ClaimError::finish(boundary.release_coordinator(), check(context))?;
    Ok(Elected::Fresh { origin: FreshRecordOrigin::Missing, inode })
}
#[cfg(test)]
mod tests;
