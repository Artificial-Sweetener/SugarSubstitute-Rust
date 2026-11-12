// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Seal only the concrete native child's authenticated creator/handler association.
#[cfg(test)]
use sugarsubstitute_contracts::crash::CaptureTarget;
use sugarsubstitute_contracts::lifecycle::RunKey;

/// Private immutable facts copied from the actual kernel-authenticated child binding.
/// No presentation, serialized metadata or numeric context can construct this seal.
#[derive(Clone, Copy)]
pub(crate) struct AuthenticatedDesktopCreator {
    run: RunKey,
    child: (u32, u64),
    creator: (u32, u64),
    handler: (u32, u64),
    core_identity: usize,
    enrollment: crate::lifecycle_pipe::LifecyclePipeDeadline,
    native_scope: sugarsubstitute_contracts::execution::ScopeId,
}
impl AuthenticatedDesktopCreator {
    #[cfg(test)]
    pub(crate) fn owner_fixture(
        core: &crate::admission::Core,
        native_scope: sugarsubstitute_contracts::execution::ScopeId,
        target: CaptureTarget,
    ) -> Result<Option<Self>, crate::lifecycle_pipe::LifecyclePipeError> {
        let Some(run) = target.desktop() else {
            return Ok(None);
        };
        Ok(Some(Self {
            run,
            child: (3, 3),
            creator: (1, 1),
            handler: (2, 2),
            core_identity: core as *const _ as usize,
            native_scope,
            enrollment: crate::lifecycle_pipe::LifecyclePipeDeadline::new(300, 10)?,
        }))
    }

    /// Called only by the concrete inherited lease after full native authentication.
    /// Backend custody remains a valid capture input and yields no Desktop creator seal.
    #[cfg(windows)]
    pub(crate) fn from_authenticated(
        binding: &crate::windows_lifecycle_pipe::captured::child::CapturedChildBinding,
    ) -> Option<Self> {
        let (facts, target) = binding.facts();
        let run = target.desktop()?;
        Some(Self {
            run,
            child: binding.child_identity(),
            creator: (facts.creator_pid, facts.creator_birth),
            handler: (facts.handler_pid, facts.handler_birth),
            core_identity: binding.core_identity(),
            enrollment: binding.enrollment(),
            native_scope: binding.native_scope(),
        })
    }
    /// Match the original actual native allocation and scope without dereferencing its identity.
    pub(crate) fn matches_native(
        self,
        core: &crate::admission::Core,
        scope: sugarsubstitute_contracts::execution::ScopeId,
    ) -> bool {
        self.core_identity == core as *const crate::admission::Core as usize
            && self.native_scope == scope
    }
    /// Bound the210s campaign by the genuine original30s enrollment cutoff.
    /// Narrowed enrollment narrows this ceiling; no current observation renews it.
    pub(crate) fn permits_campaign(
        self,
        original: sugarsubstitute_contracts::execution::CounterDeadline,
    ) -> bool {
        if original.frequency() != self.enrollment.frequency {
            return false;
        }
        let Some(ceiling) = self
            .enrollment
            .frequency
            .checked_mul(180)
            .and_then(|delta| self.enrollment.ticks.checked_add(delta))
        else {
            return false;
        };
        original.ticks() <= ceiling
    }
    /// Match sealed immutable facts; this never acquires or recreates native authority.
    pub(crate) fn matches(self, run: RunKey) -> bool {
        self.run == run
            && self.child.0 != 0
            && self.child.1 != 0
            && self.creator.0 != 0
            && self.creator.1 != 0
            && self.handler.0 != 0
            && self.handler.1 != 0
            && self.child != self.creator
            && self.child != self.handler
            && self.creator != self.handler
    }
}

#[cfg(test)]
#[path = "creator_tests.rs"]
mod tests;
