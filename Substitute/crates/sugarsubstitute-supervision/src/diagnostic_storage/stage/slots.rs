// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle retained publication stage slots without replaying uncertain effects.
use super::super::*;
use super::StageRole;
pub(in crate::diagnostic_storage) fn stage_index(
    state: &mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
) -> Result<usize, DiagnosticStorageError> {
    state
        .data_mut()?
        .stages
        .iter()
        .position(|stage| stage.id == id)
        .ok_or_else(|| error(key, Code::Closed))
}
pub(in crate::diagnostic_storage) fn stage_facts(
    state: &mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
) -> Result<DiagnosticReadFacts, DiagnosticStorageError> {
    let index = stage_index(state, id, key)?;
    state.data_mut()?.stages[index].flushed.ok_or_else(|| error(key, Code::Flush))
}
pub(in crate::diagnostic_storage) fn reserve_stage(
    state: &mut StateAccess<'_>,
    object: DiagnosticObject,
    role: StageRole,
    reserved: u64,
    key: LogicalStoreOperationKey,
) -> Result<u64, DiagnosticStorageError> {
    let namespace = state.data_mut()?.namespace;
    if !state.data_mut()?.writer
        || state.data_mut()?.stage_slots() >= DIAGNOSTIC_STAGE_BOUND
        || state.data_mut()?.live_handles() >= DIAGNOSTIC_HANDLE_BOUND
        || object.namespace().is_some_and(|value| Some(value) != namespace)
    {
        return Err(error(key, Code::Capacity));
    }
    let charged_bytes = state.data_mut()?.charged_bytes;
    let total = state
        .data_mut()?
        .stages
        .iter()
        .try_fold(charged_bytes, |sum, stage| sum.checked_add(stage.reserved))
        .and_then(|sum| sum.checked_add(reserved));
    if total.is_none_or(|sum| sum > sugarsubstitute_contracts::incident::MAX_PAYLOAD_BYTES) {
        return Err(error(key, Code::Capacity));
    }
    let id = next_lease(state, key)?;
    state.data_mut()?.stages.push(stage::RetainedStage {
        id,
        native: None,
        object,
        reserved,
        written: 0,
        flushed: None,
        recovered: None,
        recovery_predecessor: None,
        role: Some(role),
        activated: false,
        genesis: None,
    });
    Ok(id)
}
pub(in crate::diagnostic_storage) fn create_stage(
    state: &mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = stage_index(state, id, key)?;
    let (object, reserved) =
        (state.data_mut()?.stages[index].object, state.data_mut()?.stages[index].reserved);
    let native_id = state
        .call_native(|port| port.create_stage(object, key, reserved, context))?
        .map_err(|cause| native(key, Code::Write, cause))?;
    state.data_mut()?.stages[index].native = Some(native_id);
    if native_id == 0 {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)
}
pub(in crate::diagnostic_storage) fn append_stage(
    state: &mut StateAccess<'_>,
    id: u64,
    bytes: &[u8],
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = stage_index(state, id, key)?;
    let stage = &state.data_mut()?.stages[index];
    if stage.recovered.is_some() || stage.activated || bytes.len() > DIAGNOSTIC_IO_BYTES {
        return Err(error(key, Code::Busy));
    }
    let end = stage
        .written
        .checked_add(bytes.len() as u64)
        .filter(|end| *end <= stage.reserved)
        .ok_or_else(|| error(key, Code::Capacity))?;
    let (native_id, offset) = (required(stage.native, key)?, stage.written);
    state.data_mut()?.stages[index].flushed = None;
    state
        .call_native(|port| port.write(native_id, offset, bytes, context))?
        .map_err(|cause| native(key, Code::Write, cause))?;
    state.data_mut()?.stages[index].written = end;
    check(state, key, context, false)
}
pub(in crate::diagnostic_storage) fn flush_stage(
    state: &mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<DiagnosticReadFacts, DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = stage_index(state, id, key)?;
    let native_id = required(state.data_mut()?.stages[index].native, key)?;
    let actual = state
        .call_native(|port| port.flush(native_id, context))?
        .map_err(|cause| native(key, Code::Flush, cause))?;
    if actual.identity == [0; 24]
        || actual.sha256 == [0; 32]
        || actual.bytes != state.data_mut()?.stages[index].written
    {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)?;
    state.data_mut()?.stages[index].flushed = Some(actual);
    Ok(actual)
}
pub(in crate::diagnostic_storage) fn close_read(
    state: &mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = state
        .data_mut()?
        .reads
        .iter()
        .position(|read| read.id == id)
        .ok_or_else(|| error(key, Code::Closed))?;
    let native_id = required(state.data_mut()?.reads[index].native, key)?;
    state
        .call_native(|port| port.close(native_id, context))?
        .map_err(|cause| native(key, Code::Retirement, cause))?;
    check(state, key, context, false)?;
    state.data_mut()?.reads.remove(index);
    Ok(())
}
pub(in crate::diagnostic_storage) fn close_stage(
    state: &mut StateAccess<'_>,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = stage_index(state, id, key)?;
    let native_id = required(state.data_mut()?.stages[index].native, key)?;
    state
        .call_native(|port| port.close(native_id, context))?
        .map_err(|cause| native(key, Code::Retirement, cause))?;
    check(state, key, context, false)?;
    state.data_mut()?.stages.remove(index);
    Ok(())
}
pub(in crate::diagnostic_storage) fn predecessor(
    facts: DiagnosticReadFacts,
) -> DiagnosticPredecessor {
    DiagnosticPredecessor::Exact { file_identity: facts.identity, sha256: facts.sha256 }
}
pub(in crate::diagnostic_storage) fn settle_stage(
    state: &mut StateAccess<'_>,
    id: u64,
    expected: DiagnosticPredecessor,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<DiagnosticReadFacts, DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = stage_index(state, id, key)?;
    let facts = stage_facts(state, id, key)?;
    let object = state.data_mut()?.stages[index].object;
    if let Some(native_id) = state.data_mut()?.stages[index].native {
        let actual = state
            .call_native(|port| port.observe_file(native_id, context))?
            .map_err(|cause| native(key, Code::Read, cause))?;
        check(state, key, context, false)?;
        if actual != facts {
            return Err(error(key, Code::ChangedIdentity));
        }
        let target = state
            .call_native(|port| port.stage_target(native_id, object, context))?
            .map_err(|cause| native(key, Code::Read, cause))?;
        check(state, key, context, false)?;
        if let Some(actual) = target {
            if actual != facts {
                return Err(error(key, Code::ChangedIdentity));
            }
            state.data_mut()?.stages[index].activated = true;
            state
                .call_native(|port| port.close(native_id, context))?
                .map_err(|cause| native(key, Code::Retirement, cause))?;
            state.data_mut()?.stages[index].native = None;
            check(state, key, context, false)?;
        } else {
            let actual = state
                .call_native(|port| port.activate(native_id, object, expected, context))?
                .map_err(|cause| native(key, Code::ActivationUncertain, cause))?;
            if actual != facts {
                return Err(error(key, Code::ChangedIdentity));
            }
            state.data_mut()?.stages[index].activated = true;
            state.data_mut()?.stages[index].native = None;
            check(state, key, context, false)?;
        }
    }
    if !state.data_mut()?.stages[index].activated {
        return Err(error(key, Code::ActivationUncertain));
    }
    let actual = state
        .call_native(|port| port.reconcile_activation(object, predecessor(facts), context))?
        .map_err(|cause| native(key, Code::ActivationUncertain, cause))?;
    if actual != facts {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)?;
    state.data_mut()?.stages.remove(index);
    state.data_mut()?.inventory_complete = false;
    Ok(actual)
}

pub(in crate::diagnostic_storage) fn required(
    id: Option<u64>,
    key: LogicalStoreOperationKey,
) -> Result<u64, DiagnosticStorageError> {
    id.ok_or_else(|| error(key, Code::Closed))
}
