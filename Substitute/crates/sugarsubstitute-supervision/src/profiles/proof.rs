// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain nonclone authorization proofs and invalidate them before profile retirement.
use super::*;
use sugarsubstitute_contracts::execution::ExecutionId;
/// Authorize a validated actual environment folder only within its held profile lifetime.
#[must_use = "Match this proof to the current main-window generation before authorizing construction"]
pub struct EnvironmentProfileProof {
    shared: Arc<Shared>,
    run: RunKey,
    execution: Option<ExecutionId>,
    job: JobId,
    binding: attempt::Binding,
}
impl EnvironmentProfileProof {
    pub(super) fn new(owner: &WindowsProfileLease, binding: attempt::Binding, job: JobId) -> Self {
        Self {
            shared: owner.shared.clone(),
            run: owner.run,
            execution: owner.execution,
            job,
            binding,
        }
    }
    /// Match the exact retained profile/run and active native lifetime.
    pub fn validates(&self, owner: &WindowsProfileLease, identity: MainIdentity) -> bool {
        self.run == owner.run
            && self.execution == owner.execution
            && Arc::ptr_eq(&self.shared, &owner.shared)
            && self.shared.active.load(Ordering::Acquire) == HELD
            && !self.shared.closing.load(Ordering::Acquire)
            && self.binding.live(identity)
    }
    /// Return the actual finite validation job identity for generation matching.
    pub const fn job_id(&self) -> JobId {
        self.job
    }
}
/// Authorize a kernel PID/birth proved inside the exact desktop Job and held profile.
#[must_use = "Match this proof to the actual controller before authorizing navigation"]
pub struct BrowserFamilyProof {
    shared: Arc<Shared>,
    run: RunKey,
    execution: Option<ExecutionId>,
    job: JobId,
    identity: crate::native_process::Identity,
    family: crate::registry::profile::ProfileFamily,
    binding: attempt::Binding,
}
impl BrowserFamilyProof {
    pub(super) fn new(
        owner: &WindowsProfileLease,
        binding: attempt::Binding,
        job: JobId,
        identity: crate::native_process::Identity,
        family: crate::registry::profile::ProfileFamily,
    ) -> Self {
        Self {
            shared: owner.shared.clone(),
            run: owner.run,
            execution: owner.execution,
            job,
            identity,
            family,
            binding,
        }
    }
    /// Match this exact live owner/run and the controller's actual PID observation.
    pub fn validates(
        &self,
        owner: &WindowsProfileLease,
        identity: MainIdentity,
        browser_process_id: u32,
    ) -> bool {
        self.run == owner.run
            && self.execution == owner.execution
            && self.identity.pid == browser_process_id
            && self.identity.birth != 0
            && self.family.is_live()
            && self.family.identity.same_as(&owner.registry)
            && matches!(owner.launch, LaunchState::Accepted(ticket) if ticket == self.family.ticket)
            && Arc::ptr_eq(&self.shared, &owner.shared)
            && self.shared.active.load(Ordering::Acquire) == HELD
            && !self.shared.closing.load(Ordering::Acquire)
            && self.binding.live(identity)
    }
    /// Return the actual finite validation job identity for generation matching.
    pub const fn job_id(&self) -> JobId {
        self.job
    }
}

/// Own the canonical profile folder prepared on a finite worker under retained memory.
/// Units preserve Windows UTF16 exactly and exclude a terminal NUL.
#[must_use = "Keep folder facts with the actual main-window construction request"]
pub struct ProfileFolderFacts {
    units: Vec<u16>,
    run: RunKey,
    shared: Arc<Shared>,
    binding: attempt::Binding,
    job: JobId,
    execution: Option<ExecutionId>,
    _memory: sugarsubstitute_contracts::execution::MemoryClaim,
}
impl ProfileFolderFacts {
    pub(super) fn new(owner: &WindowsProfileLease, prepared: storage::PreparedFolder) -> Self {
        Self {
            shared: owner.shared.clone(),
            run: owner.run,
            units: prepared.units,
            binding: prepared.binding,
            job: prepared.job,
            execution: owner.execution,
            _memory: prepared.memory,
        }
    }
    /// Borrow canonical UTF16 for the native environment factory while this lifetime is retained.
    pub fn units(&self) -> &[u16] {
        &self.units
    }
    /// Return the actual finite preparation job for current-generation matching.
    pub const fn job_id(&self) -> JobId {
        self.job
    }
    /// Match this exact active profile owner and run before native environment construction.
    pub fn validates(&self, owner: &WindowsProfileLease, identity: MainIdentity) -> bool {
        self.run == owner.run
            && self.execution == owner.execution
            && Arc::ptr_eq(&self.shared, &owner.shared)
            && self.shared.active.load(Ordering::Acquire) == HELD
            && !self.shared.closing.load(Ordering::Acquire)
            && self.binding.live(identity)
    }
}
