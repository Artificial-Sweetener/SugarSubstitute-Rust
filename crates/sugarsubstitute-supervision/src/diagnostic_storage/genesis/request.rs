// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive a writer request from the same retained actual entry namespace.
use crate::coordinator_entry::{EntryError, VerifiedEntryNamespace, namespace::NamespaceLifetime};
use std::sync::Arc;
use sugarsubstitute_contracts::{execution::ExecutionId, incident::LogicalStoreOperationKey};
/// Retain the same live native namespace; metadata alone cannot construct this request.
/// This nonclone request grants no filesystem effects outside the admitted original job.
pub struct ProtectedDiagnosticWriteRequest {
    pub(crate) lifetime: Arc<NamespaceLifetime>,
    pub(super) execution: ExecutionId,
    pub(super) operation: LogicalStoreOperationKey,
}
impl VerifiedEntryNamespace {
    /// Borrow the actual namespace lifetime for one exact finite writer operation.
    /// # Errors
    /// Rejects a closed namespace or a different full namespace identity.
    pub fn diagnostic_write_request(
        &self,
        operation: LogicalStoreOperationKey,
    ) -> Result<ProtectedDiagnosticWriteRequest, EntryError> {
        self.validate_live()?;
        if operation.owner().namespace != self.namespace() {
            return Err(EntryError(crate::ClaimError::invalid("diagnostic_namespace_operation")));
        }
        Ok(ProtectedDiagnosticWriteRequest {
            lifetime: Arc::clone(&self.lifetime),
            execution: self.execution_owner(),
            operation,
        })
    }
}
impl ProtectedDiagnosticWriteRequest {
    pub(crate) fn execution(&self) -> ExecutionId {
        self.execution
    }
    pub(crate) fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
}
