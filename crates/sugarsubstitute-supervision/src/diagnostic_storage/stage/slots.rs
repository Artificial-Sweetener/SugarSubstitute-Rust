// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle retained publication stage slots without replaying uncertain effects.
use super::super::*;
use super::StageRole;
pub(in crate::diagnostic_storage) fn stage_index(
    state: &State,
    id: u64,
    key: LogicalStoreOperationKey,
) -> Result<usize, DiagnosticStorageError> {
    state.stages.iter().position(|stage| stage.id == id).ok_or_else(|| error(key, Code::Closed))
}
pub(in crate::diagnostic_storage) fn stage_facts(
    state: &State,
    id: u64,
    key: LogicalStoreOperationKey,
) -> Result<DiagnosticReadFacts, DiagnosticStorageError> {
    state.stages[stage_index(state, id, key)?].flushed.ok_or_else(|| error(key, Code::Flush))
}
pub(in crate::diagnostic_storage) fn reserve_stage(
    state: &mut State,
    object: DiagnosticObject,
    role: StageRole,
    reserved: u64,
    key: LogicalStoreOperationKey,
) -> Result<u64, DiagnosticStorageError> {
    if !state.writer
        || state.stages.len() >= DIAGNOSTIC_STAGE_BOUND
        || state.port.live_handles() >= DIAGNOSTIC_HANDLE_BOUND
        || object.namespace().is_some_and(|namespace| Some(namespace) != state.namespace)
    {
        return Err(error(key, Code::Capacity));
    }
    let total = state
        .stages
        .iter()
        .try_fold(state.charged_bytes, |sum, stage| sum.checked_add(stage.reserved))
        .and_then(|sum| sum.checked_add(reserved));
    if total.is_none_or(|sum| sum > sugarsubstitute_contracts::incident::MAX_PAYLOAD_BYTES) {
        return Err(error(key, Code::Capacity));
    }
    let id = next_lease(state, key)?;
    state.stages.push(stage::RetainedStage {
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
    });
    Ok(id)
}
pub(in crate::diagnostic_storage) fn create_stage(
    state: &mut State,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = stage_index(state, id, key)?;
    let (object, reserved) = (state.stages[index].object, state.stages[index].reserved);
    let native_id = state
        .port
        .create_stage(object, key, reserved, context)
        .map_err(|cause| native(key, Code::Write, cause))?;
    state.stages[index].native = Some(native_id);
    if native_id == 0 {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)
}
pub(in crate::diagnostic_storage) fn append_stage(
    state: &mut State,
    id: u64,
    bytes: &[u8],
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = stage_index(state, id, key)?;
    let stage = &state.stages[index];
    if stage.recovered.is_some() || stage.activated || bytes.len() > DIAGNOSTIC_IO_BYTES {
        return Err(error(key, Code::Busy));
    }
    let end = stage
        .written
        .checked_add(bytes.len() as u64)
        .filter(|end| *end <= stage.reserved)
        .ok_or_else(|| error(key, Code::Capacity))?;
    let (native_id, offset) = (required(stage.native, key)?, stage.written);
    state.stages[index].flushed = None;
    state
        .port
        .write(native_id, offset, bytes, context)
        .map_err(|cause| native(key, Code::Write, cause))?;
    state.stages[index].written = end;
    check(state, key, context, false)
}
pub(in crate::diagnostic_storage) fn flush_stage(
    state: &mut State,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<DiagnosticReadFacts, DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = stage_index(state, id, key)?;
    let native_id = required(state.stages[index].native, key)?;
    let actual =
        state.port.flush(native_id, context).map_err(|cause| native(key, Code::Flush, cause))?;
    if actual.identity == [0; 24]
        || actual.sha256 == [0; 32]
        || actual.bytes != state.stages[index].written
    {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)?;
    state.stages[index].flushed = Some(actual);
    Ok(actual)
}
pub(in crate::diagnostic_storage) fn close_read(
    state: &mut State,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = state
        .reads
        .iter()
        .position(|read| read.id == id)
        .ok_or_else(|| error(key, Code::Closed))?;
    let native_id = required(state.reads[index].native, key)?;
    state.port.close(native_id, context).map_err(|cause| native(key, Code::Retirement, cause))?;
    check(state, key, context, false)?;
    state.reads.remove(index);
    Ok(())
}
pub(in crate::diagnostic_storage) fn close_stage(
    state: &mut State,
    id: u64,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = stage_index(state, id, key)?;
    let native_id = required(state.stages[index].native, key)?;
    state.port.close(native_id, context).map_err(|cause| native(key, Code::Retirement, cause))?;
    check(state, key, context, false)?;
    state.stages.remove(index);
    Ok(())
}
pub(in crate::diagnostic_storage) fn predecessor(
    facts: DiagnosticReadFacts,
) -> DiagnosticPredecessor {
    DiagnosticPredecessor::Exact { file_identity: facts.identity, sha256: facts.sha256 }
}
pub(in crate::diagnostic_storage) fn settle_stage(
    state: &mut State,
    id: u64,
    expected: DiagnosticPredecessor,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<DiagnosticReadFacts, DiagnosticStorageError> {
    check(state, key, context, false)?;
    let index = stage_index(state, id, key)?;
    let facts = stage_facts(state, id, key)?;
    let object = state.stages[index].object;
    if let Some(native_id) = state.stages[index].native {
        let actual = state
            .port
            .observe_file(native_id, context)
            .map_err(|cause| native(key, Code::Read, cause))?;
        check(state, key, context, false)?;
        if actual != facts {
            return Err(error(key, Code::ChangedIdentity));
        }
        let target = state
            .port
            .stage_target(native_id, object, context)
            .map_err(|cause| native(key, Code::Read, cause))?;
        check(state, key, context, false)?;
        if let Some(actual) = target {
            if actual != facts {
                return Err(error(key, Code::ChangedIdentity));
            }
            state.stages[index].activated = true;
            state
                .port
                .close(native_id, context)
                .map_err(|cause| native(key, Code::Retirement, cause))?;
            state.stages[index].native = None;
            check(state, key, context, false)?;
        } else {
            let actual = state
                .port
                .activate(native_id, object, expected, context)
                .map_err(|cause| native(key, Code::ActivationUncertain, cause))?;
            if actual != facts {
                return Err(error(key, Code::ChangedIdentity));
            }
            state.stages[index].activated = true;
            state.stages[index].native = None;
            check(state, key, context, false)?;
        }
    }
    if !state.stages[index].activated {
        return Err(error(key, Code::ActivationUncertain));
    }
    let actual = state
        .port
        .reconcile_activation(object, predecessor(facts), context)
        .map_err(|cause| native(key, Code::ActivationUncertain, cause))?;
    if actual != facts {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)?;
    state.stages.remove(index);
    state.inventory_complete = false;
    Ok(actual)
}

pub(in crate::diagnostic_storage) fn required(
    id: Option<u64>,
    key: LogicalStoreOperationKey,
) -> Result<u64, DiagnosticStorageError> {
    id.ok_or_else(|| error(key, Code::Closed))
}
