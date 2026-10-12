// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct complete actual source observations while retaining their exact slot origin.
use super::*;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceOrigin {
    Read(u64),
    Stage(u64),
}
pub(super) fn read_by_id(
    storage: &DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    id: u64,
    context: &JobContext,
) -> Result<DiagnosticRecoverySource, DiagnosticStorageError> {
    let facts = {
        let state = storage.check(operation, context)?;
        state
            .reads
            .iter()
            .find(|read| read.id == id)
            .and_then(|read| read.facts)
            .ok_or_else(|| error(operation.key, Code::Closed))?
    };
    let mut lease =
        DiagnosticReadLease { shared: storage.shared.clone(), operation: operation.key, id, facts };
    whole(storage, &mut lease, operation, context)
}
pub(super) fn stage_by_id(
    storage: &DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    id: u64,
    context: &JobContext,
) -> Result<DiagnosticRecoverySource, DiagnosticStorageError> {
    let mut state = storage.check(operation, context)?;
    let retained = state
        .stages
        .iter()
        .find(|stage| stage.id == id)
        .ok_or_else(|| error(operation.key, Code::Closed))?;
    let facts = retained.flushed.ok_or_else(|| error(operation.key, Code::Flush))?;
    let native_id = retained.native.ok_or_else(|| error(operation.key, Code::Closed))?;
    let object = retained.object;
    if facts.bytes > DIAGNOSTIC_IO_BYTES as u64 {
        return Err(error(operation.key, Code::Capacity));
    }
    let target = state
        .port
        .migration_stage_target(native_id, object, context)
        .map_err(|cause| native(operation.key, Code::Read, cause))?;
    check(&mut state, operation.key, context, false)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(facts.bytes as usize).map_err(|cause| {
        native(operation.key, Code::Capacity, ClaimError::new("diagnostic_source_bytes", cause))
    })?;
    bytes.resize(facts.bytes as usize, 0);
    if state
        .port
        .read(native_id, 0, &mut bytes, context)
        .map_err(|cause| native(operation.key, Code::Read, cause))?
        != bytes.len()
        || <[u8; 32]>::from(Sha256::digest(&bytes)) != facts.sha256
    {
        return Err(error(operation.key, Code::ChangedIdentity));
    }
    check(&mut state, operation.key, context, false)?;
    let namespace = state.namespace.ok_or_else(|| error(operation.key, Code::Closed))?;
    let (active_class, name_digest) =
        crate::protected_storage::record_file::diagnostic_object_binding(object, namespace);
    if target.is_some_and(|actual| actual != facts) {
        return Err(error(operation.key, Code::ChangedIdentity));
    }
    Ok(DiagnosticRecoverySource {
        bytes,
        facts,
        object: Some(object),
        class: if target.is_some() { active_class } else { DiagnosticEntryClass::Stage },
        name_digest,
        origin: SourceOrigin::Stage(id),
        native: native_id,
    })
}
pub(super) fn whole(
    storage: &DiagnosticStorage,
    lease: &mut DiagnosticReadLease,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> Result<DiagnosticRecoverySource, DiagnosticStorageError> {
    if !Arc::ptr_eq(&storage.shared, &lease.shared) || lease.operation != operation.key {
        return Err(error(operation.key, Code::InvalidIdentity));
    }
    let (native_id, object, class, name_digest) = {
        let state = storage.check(operation, context)?;
        let retained = state
            .reads
            .iter()
            .find(|value| value.id == lease.id)
            .ok_or_else(|| error(operation.key, Code::Closed))?;
        if retained.facts != Some(lease.facts()) || lease.facts().bytes > DIAGNOSTIC_IO_BYTES as u64
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        let namespace = state.namespace.ok_or_else(|| error(operation.key, Code::Closed))?;
        let binding = match (retained.object, retained.candidate) {
            (Some(object), candidate) => {
                let actual = crate::protected_storage::record_file::diagnostic_object_binding(
                    object, namespace,
                );
                if candidate.is_some_and(|observed| observed != actual) {
                    return Err(error(operation.key, Code::InvalidIdentity));
                }
                actual
            }
            (None, Some(candidate)) => candidate,
            _ => return Err(error(operation.key, Code::InvalidIdentity)),
        };
        (
            retained.native.ok_or_else(|| error(operation.key, Code::Closed))?,
            retained.object,
            binding.0,
            binding.1,
        )
    };
    let size =
        usize::try_from(lease.facts().bytes).map_err(|_| error(operation.key, Code::Capacity))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size).map_err(|cause| {
        native(operation.key, Code::Capacity, ClaimError::new("diagnostic_recovery_bytes", cause))
    })?;
    bytes.resize(size, 0);
    if lease.read_chunk(0, &mut bytes, context)? != size
        || <[u8; 32]>::from(Sha256::digest(&bytes)) != lease.facts().sha256
    {
        return Err(error(operation.key, Code::ChangedIdentity));
    }
    Ok(DiagnosticRecoverySource {
        bytes,
        facts: lease.facts(),
        object,
        class,
        name_digest,
        origin: SourceOrigin::Read(lease.id),
        native: native_id,
    })
}
pub(super) fn canonical(
    source: &DiagnosticRecoverySource,
    object: DiagnosticObject,
    namespace: NamespaceId,
) -> bool {
    source.object.is_none_or(|actual| actual == object)
        && (source.class, source.name_digest)
            == crate::protected_storage::record_file::diagnostic_object_binding(object, namespace)
}
pub(super) fn retained_source(
    state: &mut State,
    source: &DiagnosticRecoverySource,
    key: LogicalStoreOperationKey,
    context: &JobContext,
) -> Result<usize, DiagnosticStorageError> {
    check(state, key, context, false)?;
    let actual = state
        .port
        .observe_file(source.native, context)
        .map_err(|cause| native(key, Code::Read, cause))?;
    if actual != source.facts {
        return Err(error(key, Code::ChangedIdentity));
    }
    check(state, key, context, false)?;
    let index = match source.origin {
        SourceOrigin::Stage(id) => {
            let object = source.object.ok_or_else(|| error(key, Code::InvalidIdentity))?;
            let target = state
                .port
                .migration_stage_target(source.native, object, context)
                .map_err(|cause| native(key, Code::Read, cause))?;
            check(state, key, context, false)?;
            let (active, digest) = crate::protected_storage::record_file::diagnostic_object_binding(
                object,
                key.owner().namespace,
            );
            if source.name_digest != digest
                || source.class
                    != if target.is_some() { active } else { DiagnosticEntryClass::Stage }
                || target.is_some_and(|facts| facts != source.facts)
            {
                return Err(error(key, Code::ChangedIdentity));
            }
            state.stages.iter().position(|value| {
                value.id == id
                    && value.object == object
                    && value.native == Some(source.native)
                    && value.flushed == Some(source.facts)
            })
        }
        SourceOrigin::Read(_) => state.reads.iter().position(|value| {
            source.origin == SourceOrigin::Read(value.id)
                && value.native == Some(source.native)
                && value.facts == Some(source.facts)
                && value.object == source.object
        }),
    };
    index.ok_or_else(|| error(key, Code::ChangedIdentity))
}
