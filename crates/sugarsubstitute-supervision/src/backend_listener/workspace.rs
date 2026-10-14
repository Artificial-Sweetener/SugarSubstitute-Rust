// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep one complete aligned TCP snapshot buffer under its original readiness admission.
use super::{RegistryError, table};
use sugarsubstitute_contracts::execution::{ExecutionId, JobContext, MemoryClaim, ScopeId};

/// Fixed TCP storage, leaving eight MiB for HTTP/parsing and 64 KiB for an attached peer.
pub const TCP_OWNER_WORKSPACE_BYTES: u64 = 8 * 1024 * 1024 - 64 * 1024;

/// Noncloneable reusable query storage; its claim survives until the actual buffer is freed.
/// This grants neither listener authority nor managed/attached process ownership.
pub struct TcpOwnerWorkspace {
    pub(super) words: Vec<u32>,
    execution: ExecutionId,
    scope: ScopeId,
    _claim: MemoryClaim,
}
impl TcpOwnerWorkspace {
    /// Fund and allocate one fixed buffer before this readiness owner's first native effect.
    /// Subsequent finite phases reuse it in the same actual executor/scope.
    /// # Errors
    /// Rejects invalid current control, insufficient original admission or allocation failure.
    pub fn new(context: &JobContext) -> Result<Self, RegistryError> {
        table::check(context)?;
        let claim = context
            .memory()
            .claim(TCP_OWNER_WORKSPACE_BYTES)
            .map_err(|cause| RegistryError::new("listener_table_memory", cause))?;
        let count = TCP_OWNER_WORKSPACE_BYTES as usize / size_of::<u32>();
        let mut words = Vec::new();
        words
            .try_reserve_exact(count)
            .map_err(|cause| RegistryError::new("listener_table_allocation", cause))?;
        if words.capacity() != count {
            return Err(RegistryError::invalid("listener_table_capacity"));
        }
        words.resize(count, 0);
        table::check(context)?;
        Ok(Self { words, execution: context.owner_id(), scope: context.scope_id(), _claim: claim })
    }
    pub(super) fn check(&self, context: &JobContext) -> Result<(), RegistryError> {
        if self.execution != context.owner_id() || self.scope != context.scope_id() {
            return Err(RegistryError::invalid("listener_table_owner"));
        }
        table::check(context)
    }
}
