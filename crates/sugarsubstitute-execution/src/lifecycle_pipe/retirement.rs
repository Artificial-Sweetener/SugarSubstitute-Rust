// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deliver checked physical settlement independently of the original operation outcome.
use super::{LifecyclePipeControl, SharedRef, lock};
use sugarsubstitute_contracts::{
    execution::{ExecutionId, JobSpec, ScopeId},
    lifecycle::RunKey,
};

/// Facts stored inline only after the worker drops its settled native owner and buffers.
pub(super) struct CheckedRetirement {
    pub(super) execution: ExecutionId,
    pub(super) scope: ScopeId,
    pub(super) spec: JobSpec,
    pub(super) run: Option<RunKey>,
}

/// Checked settlement of one actual pipe's handles, callbacks, buffers and launch pins.
///
/// The operation may have failed. This proof neither replaces its original outcome
/// nor grants process-family, profile, capture-handler or backend retirement authority.
/// The original shared allocation and its outside metadata charge remain retained.
/// No constructor or clone is exposed; delivery transfers once from the actual control.
pub struct LifecyclePipeRetirement {
    shared: SharedRef,
    facts: CheckedRetirement,
}

impl LifecyclePipeRetirement {
    /// Match the actual retained session allocation, rather than copied matching IDs.
    pub fn matches(&self, control: &LifecyclePipeControl) -> bool {
        self.shared.same_allocation(&control.inner.shared)
    }

    /// Return the original execution owner's identity.
    pub const fn execution_id(&self) -> ExecutionId {
        self.facts.execution
    }

    /// Return the original admitted native scope.
    pub const fn scope_id(&self) -> ScopeId {
        self.facts.scope
    }

    /// Return the immutable original job specification; no deadline is renewed.
    pub const fn spec(&self) -> JobSpec {
        self.facts.spec
    }

    /// Return the exact original desktop binding, if this was a desktop session.
    pub const fn run(&self) -> Option<RunKey> {
        self.facts.run
    }
}

impl LifecyclePipeControl {
    /// Transfer checked physical settlement once, independently of operation success.
    ///
    /// Returns None until native retirement succeeds, every loan and launch pin returns,
    /// and the actual worker drops its buffers and native lease. Native close failure,
    /// panic, cancellation intent, a terminal notice and memory disposal cannot mint it.
    /// This cache-only transfer invokes no clock/provider/native call or new allocation.
    pub fn take_checked_retirement(&self) -> Option<LifecyclePipeRetirement> {
        let facts = lock(&self.inner.shared.state).checked_retirement.take()?;
        Some(LifecyclePipeRetirement { shared: self.inner.shared.clone(), facts })
    }
}
