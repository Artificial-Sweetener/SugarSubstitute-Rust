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
    purpose: Purpose,
    _claim: MemoryClaim,
}

/// Separate readiness observations from ordinary output acquisition.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Purpose {
    Lifecycle,
    Output,
}

impl Purpose {
    pub(super) fn check(self, context: &JobContext) -> Result<(), RegistryError> {
        match self {
            Self::Lifecycle => table::check(context),
            Self::Output => table::check_output(context),
        }
    }
}

impl TcpOwnerWorkspace {
    /// Fund and allocate one fixed buffer before this readiness owner's first native effect.
    /// Subsequent finite phases reuse it in the same actual executor/scope.
    /// # Errors
    /// Rejects invalid current control, insufficient original admission or allocation failure.
    pub fn new(context: &JobContext) -> Result<Self, RegistryError> {
        Self::create(context, Purpose::Lifecycle)
    }
    /// Fund a connected-peer snapshot for ordinary Comfy output acquisition.
    /// It cannot qualify managed readiness or query listening sockets.
    ///
    /// # Errors
    /// Rejects non-ordinary work, invalid controls, insufficient admission or allocation failure.
    pub fn for_output(context: &JobContext) -> Result<Self, RegistryError> {
        Self::create(context, Purpose::Output)
    }
    fn create(context: &JobContext, purpose: Purpose) -> Result<Self, RegistryError> {
        purpose.check(context)?;
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
        purpose.check(context)?;
        Ok(Self {
            words,
            execution: context.owner_id(),
            scope: context.scope_id(),
            purpose,
            _claim: claim,
        })
    }
    pub(super) fn check(&self, context: &JobContext) -> Result<(), RegistryError> {
        if self.execution != context.owner_id() || self.scope != context.scope_id() {
            return Err(RegistryError::invalid("listener_table_owner"));
        }
        self.purpose.check(context)
    }
    pub(super) fn purpose(&self) -> Purpose {
        self.purpose
    }
    pub(super) fn require_lifecycle(&self, context: &JobContext) -> Result<(), RegistryError> {
        self.check(context)?;
        if self.purpose != Purpose::Lifecycle {
            return Err(RegistryError::invalid("listener_lifecycle_workspace"));
        }
        Ok(())
    }
}
