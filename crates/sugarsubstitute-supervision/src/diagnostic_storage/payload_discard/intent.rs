// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one sealed group with independent per-file effect and closure facts.
use super::*;
pub(in crate::diagnostic_storage) struct PayloadIntent {
    pub(super) id: u64,
    pub(super) subject_digest: [u8; 32],
    pub(super) reserved_files: usize,
    pub(super) selection: Option<DiagnosticPayloadDiscardSelection>,
    pub(super) original_read: Option<u64>,
    pub(super) pending_journal_read: Option<u64>,
    pub(super) candidate_stage: Option<u64>,
    pub(super) journal_stage: Option<u64>,
    pub(super) journal: Option<RetainedVictim>,
    pub(super) victims: Vec<RetainedVictim>,
    pub(super) validating: bool,
    pub(super) decision: Option<DiagnosticPayloadDiscardDecision>,
    pub(super) journal_bytes: Option<Vec<u8>>,
    pub(super) journal_active: Option<DiagnosticReadFacts>,
    pub(super) candidate_active: Option<DiagnosticReadFacts>,
}
pub(in crate::diagnostic_storage) struct RetainedVictim {
    pub(super) object: DiagnosticObject,
    pub(super) facts: Option<DiagnosticReadFacts>,
    pub(super) native: Option<NativeId>,
    pub(super) attempted: bool,
    pub(super) issued: bool,
    pub(super) closed: bool,
    pub(super) absent: bool,
}
impl RetainedVictim {
    pub(super) fn new(object: DiagnosticObject) -> Self {
        Self {
            object,
            facts: None,
            native: None,
            attempted: false,
            issued: false,
            closed: false,
            absent: false,
        }
    }
}
pub(super) fn intent<'a>(
    state: &'a mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
) -> Result<&'a mut PayloadIntent, DiagnosticStorageError> {
    state
        .data_mut()?
        .payload
        .as_deref_mut()
        .filter(|intent| intent.id == id)
        .ok_or_else(|| error(key, Code::InvalidIdentity))
}
pub(super) fn pin(
    shared: &Shared,
    other: &Shared,
    current: LogicalStoreOperationKey,
    operation: &DiagnosticStorageOperation,
) -> Result<(), DiagnosticStorageError> {
    if current != operation.key
        || !Shared::ptr_eq(shared, other)
        || !Shared::ptr_eq(shared, &operation.shared)
    {
        return Err(error(operation.key, Code::InvalidIdentity));
    }
    Ok(())
}
pub(in crate::diagnostic_storage) fn pins_read(
    state: &mut StateAccess<'_>,
    id: u64,
) -> Result<bool, DiagnosticStorageError> {
    Ok(state.data_mut()?.payload.as_ref().is_some_and(|intent| {
        intent.original_read == Some(id) || intent.pending_journal_read == Some(id)
    }))
}

pub(super) fn capacity(
    state: &mut StateAccess<'_>,
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    // The future Journal creation and readback overlap remain reserved while victims live.
    if state
        .data_mut()?
        .retained_files()
        .checked_add(MAX_ATTACHMENTS + 2)
        .is_none_or(|count| count > 16)
        || state
            .data_mut()?
            .live_handles()
            .checked_add(MAX_ATTACHMENTS + 2)
            .is_none_or(|count| count > DIAGNOSTIC_HANDLE_BOUND)
        || state.data_mut()?.stages.len() >= DIAGNOSTIC_STAGE_BOUND
    {
        return Err(error(key, Code::Capacity));
    }
    Ok(())
}
pub(super) fn revalidate_victims(
    state: &mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    let count = intent(state, id, key)?.victims.len();
    for index in 0..count {
        check(state, key, context, false)?;
        let victim = &intent(state, id, key)?.victims[index];
        let (native_id, facts) =
            (required(victim.native, key)?, victim.facts.ok_or_else(|| error(key, Code::Closed))?);
        let actual = state
            .call_native(|port| port.observe_file(native_id, context))?
            .map_err(|cause| native(key, Code::Read, cause))?;
        check(state, key, context, false)?;
        if actual != facts {
            return Err(error(key, Code::ChangedIdentity));
        }
    }
    Ok(())
}

pub(in crate::diagnostic_storage) fn admit_read(
    state: &mut StateAccess<'_>,
    binding: (DiagnosticEntryClass, [u8; 32]),
    key: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    let Some(intent) = &state.data_mut()?.payload else {
        return Ok(());
    };
    if matches!(
        binding.0,
        DiagnosticEntryClass::Summary | DiagnosticEntryClass::Payload | DiagnosticEntryClass::Stage
    ) && binding.1 == intent.subject_digest
    {
        return Err(error(key, Code::Busy));
    }
    let reserved_files = intent.reserved_files;
    if state
        .data_mut()?
        .retained_files()
        .checked_add(reserved_files)
        .and_then(|count| count.checked_add(1))
        .is_none_or(|count| count > 16)
        || state
            .data_mut()?
            .live_handles()
            .checked_add(reserved_files)
            .and_then(|count| count.checked_add(1))
            .is_none_or(|count| count > DIAGNOSTIC_HANDLE_BOUND)
    {
        return Err(error(key, Code::Capacity));
    }
    Ok(())
}
