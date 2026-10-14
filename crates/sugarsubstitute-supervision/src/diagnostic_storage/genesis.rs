// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Mint empty-ledger publication authority only from actual native directory creation.
mod request;
use super::*;
pub use request::ProtectedDiagnosticWriteRequest;

/// Distinguish actual native creation from opening an existing protected store.
pub enum DiagnosticStoreOpening {
    /// Existing bytes require mandatory ledger validation, including absence failure.
    Existing,
    /// Retain the one-use live creation proof through exact ledger activation.
    Created(DiagnosticStoreGenesis),
}
/// Retain the exact fresh native namespace and its original operation custody.
/// No observed path, absence, identity tuple or caller Boolean constructs this proof.
pub struct DiagnosticStoreGenesis {
    shared: Shared,
    operation: LogicalStoreOperationKey,
    identity: [u8; 24],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GenesisContent {
    pub(super) sha256: [u8; 32],
    pub(super) bytes: u64,
}
pub(super) struct RetainedGenesis {
    identity: [u8; 24],
    pub(super) content: Option<GenesisContent>,
    pub(super) stage: Option<u64>,
    pub(super) committed: bool,
}
/// Facts produced only by the private native boundary after checked directory open.
#[derive(Clone, Copy)]
pub(crate) enum NativeOpening {
    Existing,
    Created([u8; 24]),
}
impl DiagnosticStorage {
    /// Resolve through the same live authenticated entry namespace, under this operation.
    /// # Errors
    /// Rejects foreign execution/full operation, closed source, repeated resolution,
    /// changed native identity/protection, or original control failure. Partial native
    /// outputs remain owned; absence in an existing namespace never mints genesis.
    pub fn resolve_namespace(
        &mut self,
        request: &ProtectedDiagnosticWriteRequest,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticStoreOpening, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        if request.operation != operation.key
            || request.execution != context.owner_id()
            || state.data_mut()?.namespace.is_some()
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        state.data_mut()?.allocation.bind_namespace(request).map_err(|cause| {
            DiagnosticStorageError::native(
                Some(operation.key),
                Code::Admission,
                crate::ClaimError::diagnostic_allocation(cause),
            )
        })?;
        let opening = state
            .call_native(|port| port.resolve_namespace(request, context))?
            .map_err(|cause| native(operation.key, Code::ForeignProtection, cause))?;
        self.adopt_opening(state, opening, operation, context)
    }
    pub(super) fn adopt_opening(
        &self,
        mut state: StateAccess<'_>,
        opening: NativeOpening,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticStoreOpening, DiagnosticStorageError> {
        state.data_mut()?.namespace = Some(operation.key.owner().namespace);
        if let NativeOpening::Created(identity) = opening {
            if identity == [0; 24] {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            // Retain the actual creation observation before fallible post-control.
            state.data_mut()?.genesis =
                Some(RetainedGenesis { identity, content: None, stage: None, committed: false });
            check(&mut state, operation.key, context, false)?;
            Ok(DiagnosticStoreOpening::Created(DiagnosticStoreGenesis {
                shared: self.shared.clone(),
                operation: operation.key,
                identity,
            }))
        } else {
            check(&mut state, operation.key, context, false)?;
            Ok(DiagnosticStoreOpening::Existing)
        }
    }
    /// Reserve the one empty-ledger intent before any native stage creation.
    /// The repository supplies strict encoded empty Ledger bytes and their full hash.
    /// # Errors
    /// Rejects foreign/finalized proof, nonempty or incomplete actual inventory,
    /// mismatched content, exhausted capacity, and repeated stage admission.
    pub fn stage_genesis(
        &mut self,
        genesis: &mut DiagnosticStoreGenesis,
        expected_sha256: [u8; 32],
        bytes: u64,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticStage, DiagnosticStorageError> {
        if !Shared::ptr_eq(&self.shared, &genesis.shared) || genesis.operation != operation.key {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let mut state = self.check(operation, context)?;
        if state
            .data_mut()?
            .genesis
            .as_ref()
            .is_none_or(|retained| retained.identity != genesis.identity)
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        drop(state);
        self.stage_with_purpose(
            DiagnosticObject::StoreState,
            bytes,
            Some(GenesisContent { sha256: expected_sha256, bytes }),
            operation,
            context,
        )
    }
    /// Reborrow the same already-issued stage after a late original control failure.
    /// # Errors
    /// Rejects missing/partial native creation or a different owner/original operation.
    /// It creates no inode, renews no deadline and never issues a second stage.
    pub fn retained_genesis_stage(
        &mut self,
        genesis: &mut DiagnosticStoreGenesis,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticStage, DiagnosticStorageError> {
        if !Shared::ptr_eq(&self.shared, &genesis.shared) || genesis.operation != operation.key {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let mut state = self.check(operation, context)?;
        let retained =
            state.data_mut()?.genesis.as_ref().ok_or_else(|| error(operation.key, Code::Closed))?;
        if retained.identity != genesis.identity || retained.committed {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let id = retained.stage.ok_or_else(|| error(operation.key, Code::Closed))?;
        if !state.data_mut()?.stages.iter().any(|stage| stage.id == id && stage.native.is_some()) {
            return Err(error(operation.key, Code::Closed));
        }
        Ok(DiagnosticStage { shared: self.shared.clone(), operation: operation.key, id })
    }
}
pub(super) fn validate_issue(
    state: &mut StateAccess<'_>,
    key: LogicalStoreOperationKey,
    content: GenesisContent,
) -> Result<(), DiagnosticStorageError> {
    let data = state.data_mut()?;
    let retained = data.genesis.as_ref().ok_or_else(|| error(key, Code::InvalidIdentity))?;
    if retained.committed
        || retained.stage.is_some()
        || !data.stages.is_empty()
        || !data.scan_entries.is_empty()
        || content.bytes == 0
        || content.bytes > 65536
        || content.sha256 == [0; 32]
    {
        return Err(error(key, Code::InvalidIdentity));
    }
    Ok(())
}
pub(super) fn validate_activation(
    state: &State,
    key: LogicalStoreOperationKey,
    id: u64,
    facts: DiagnosticReadFacts,
) -> Result<(), DiagnosticStorageError> {
    let retained = state.genesis.as_ref().ok_or_else(|| error(key, Code::InvalidIdentity))?;
    let stage =
        state.stages.iter().find(|stage| stage.id == id).ok_or_else(|| error(key, Code::Closed))?;
    let content = stage.genesis.ok_or_else(|| error(key, Code::InvalidIdentity))?;
    if retained.committed
        || retained.stage != Some(id)
        || retained.content != Some(content)
        || content.sha256 != facts.sha256
        || content.bytes != facts.bytes
    {
        return Err(error(key, Code::InvalidIdentity));
    }
    Ok(())
}

pub(super) fn reconcile(
    state: &mut State,
    key: LogicalStoreOperationKey,
    facts: DiagnosticReadFacts,
) -> Result<(), DiagnosticStorageError> {
    let Some(retained) = state.genesis.as_mut() else {
        return Ok(());
    };
    if retained.committed {
        return Ok(());
    }
    let Some(content) = retained.content else {
        return Err(error(key, Code::InvalidIdentity));
    };
    if content.sha256 != facts.sha256 || content.bytes != facts.bytes {
        return Err(error(key, Code::ChangedIdentity));
    }
    let id = retained.stage.ok_or_else(|| error(key, Code::InvalidIdentity))?;
    retained.committed = true;
    state.stages.retain(|stage| stage.id != id);
    state.inventory_complete = false;
    Ok(())
}
