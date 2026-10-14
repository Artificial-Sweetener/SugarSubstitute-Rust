// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow the genuine namespace lifetime without reconstructing it from public facts.
use crate::coordinator_entry::{EntryError, VerifiedEntryNamespace};
use std::sync::Arc;
use sugarsubstitute_contracts::{
    execution::ExecutionId, incident::LogicalStoreOperationKey, lifecycle::NamespaceId,
};

/// Retain the same protected namespace and current transaction for existing-only reads.
/// Construction performs cached checks. Finite reader work separately revalidates
/// the genuine native namespace and its original execution origin before any effect.
pub struct ProtectedCommittedReadRequest {
    pub(crate) lifetime: Arc<crate::coordinator_entry::NamespaceLifetime>,
    pub(super) namespace: NamespaceId,
    pub(super) execution: ExecutionId,
    pub(super) operation: LogicalStoreOperationKey,
}
impl VerifiedEntryNamespace {
    /// Retain the original namespace for one committed-reader construction.
    /// This shares its existing lifetime and charge; it creates no native authority,
    /// path, new control allocation, or filesystem object.
    /// # Errors
    /// Rejects closed namespace admission and an operation in another namespace.
    pub fn committed_read_request(
        &self,
        operation: LogicalStoreOperationKey,
    ) -> Result<ProtectedCommittedReadRequest, EntryError> {
        self.validate_live()?;
        if operation.owner().namespace != self.namespace() {
            return Err(EntryError(crate::ClaimError::invalid("committed_read_namespace")));
        }
        Ok(ProtectedCommittedReadRequest {
            lifetime: Arc::clone(&self.lifetime),
            namespace: self.namespace(),
            execution: self.execution,
            operation,
        })
    }
}
impl ProtectedCommittedReadRequest {
    /// Observe the current full operation as inert metadata.
    pub fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Observe the actual retained namespace as inert metadata.
    pub fn namespace(&self) -> NamespaceId {
        self.namespace
    }
    /// Observe the actual namespace preparation execution owner.
    pub fn execution_owner(&self) -> ExecutionId {
        self.execution
    }
}
