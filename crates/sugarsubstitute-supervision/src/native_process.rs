// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep each owned Job/process/thread and complete inventory alive through partial failure.
//! Integer handle slots are unique owned identities, never caller-supplied authority.
//! Creation and normal cleanup run on finite execution workers. Owner-loss closure
//! may run on the last owner's thread: closing the KILL_ON_CLOSE Job is authorized
//! containment and does not manufacture settlement. A failed close intentionally
//! retains only its bounded native storage and lease; it is not retryable state.
use crate::registry_input::output::{OutputCancellation, OutputPin};
use crate::{FamilyBinding, RegistryError, WindowsLaunchInput};
use std::{
    os::windows::io::{AsRawHandle, OwnedHandle},
    sync::Arc,
};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use sugarsubstitute_contracts::lifecycle::{ProcessKey, ResourceKey, RunKey};
use sugarsubstitute_execution::windows_fixture::FixtureChildEvents;
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecycleDesktopEnrollment, LifecyclePipeBootstrap,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE};

pub(crate) const MAX_MEMBERS: usize = 256;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Identity {
    pub(crate) pid: u32,
    pub(crate) birth: u64,
}
pub(crate) struct Member {
    pub(crate) handle: Option<usize>,
    pub(crate) identity: Identity,
    pub(crate) exited: bool,
}
pub(crate) struct Family {
    pub(crate) job: Option<usize>,
    pub(crate) thread: Option<usize>,
    pub(crate) watch_duplicate: Option<usize>,
    pub(crate) members: [Option<Member>; MAX_MEMBERS],
    pub(crate) count: usize,
    pub(crate) root: Option<Identity>,
    pub(crate) attributes: Vec<usize>,
    pub(crate) query: Vec<usize>,
    pub(crate) attributes_initialized: bool,
    pub(crate) job_list: [usize; 1],
    pub(crate) fixture_events: Option<FixtureChildEvents>,
    pub(crate) lifecycle_bootstrap: Option<LifecyclePipeBootstrap>,
    pub(crate) lifecycle_enrollment: Option<LifecycleDesktopEnrollment>,
    pub(crate) lifecycle_run: Option<RunKey>,
    pub(crate) output_pin: Option<OutputPin>,
    pub(crate) output_cancel: Option<OutputCancellation>,
    pub(crate) output_resource: Option<ResourceKey>,
    pub(crate) inherited_handles: [usize; 4],
    pub(crate) inherited_count: usize,
    pub(crate) stdio_handles: [usize; 3],
    pub(crate) sealed: bool,
    pub(crate) incomplete: bool,
    pub(crate) forced: bool,
    pub(crate) resumed: bool,
    pub(crate) closure_committed: bool,
    _memory: MemoryClaim,
}
impl Family {
    pub(crate) fn new(memory: &MemoryLease) -> Result<Box<Self>, RegistryError> {
        let claim = memory
            .claim(crate::registry_input::FAMILY_STORAGE_BYTES)
            .map_err(|cause| RegistryError::new("family_memory", cause))?;
        let mut attributes = Vec::new();
        attributes
            .try_reserve_exact(2048)
            .map_err(|cause| RegistryError::new("attribute_storage", cause))?;
        attributes.resize(2048, 0);
        let mut query = Vec::new();
        query
            .try_reserve_exact(MAX_MEMBERS + 2)
            .map_err(|cause| RegistryError::new("inventory_storage", cause))?;
        query.resize(MAX_MEMBERS + 2, 0);
        let actual = std::mem::size_of::<Self>()
            + (attributes.capacity() + query.capacity()) * std::mem::size_of::<usize>();
        if actual as u64 > claim.bytes() {
            return Err(RegistryError::invalid("family_capacity"));
        }
        Ok(Box::new(Self {
            job: None,
            thread: None,
            watch_duplicate: None,
            members: std::array::from_fn(|_| None),
            count: 0,
            root: None,
            attributes,
            query,
            attributes_initialized: false,
            job_list: [0],
            fixture_events: None,
            lifecycle_bootstrap: None,
            lifecycle_enrollment: None,
            lifecycle_run: None,
            output_pin: None,
            output_cancel: None,
            output_resource: None,
            inherited_handles: [0; 4],
            inherited_count: 0,
            stdio_handles: [0; 3],
            sealed: false,
            incomplete: false,
            forced: false,
            resumed: false,
            closure_committed: false,
            _memory: claim,
        }))
    }
    pub(crate) fn member(&self, index: usize) -> Result<&Member, RegistryError> {
        self.members
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| RegistryError::invalid("member_identity"))
    }
    pub(crate) fn record_exit(&mut self, identity: Identity) -> Result<(), RegistryError> {
        let member = self
            .members
            .iter_mut()
            .flatten()
            .find(|member| member.identity == identity)
            .ok_or_else(|| RegistryError::invalid("member_exit_identity"))?;
        member.exited = true;
        Ok(())
    }
    pub(crate) fn next_member(&self, root_watch_active: bool) -> Option<(usize, Identity)> {
        self.members
            .iter()
            .enumerate()
            .filter_map(|(index, member)| member.as_ref().map(|m| (index, m)))
            .find(|(_, member)| {
                !member.exited && (!root_watch_active || Some(member.identity) != self.root)
            })
            .map(|(index, member)| (index, member.identity))
    }
    pub(crate) fn all_exited(&self) -> bool {
        self.members.iter().flatten().all(|member| member.exited)
    }
}
pub(crate) struct Inventory {
    pub(crate) empty: bool,
    pub(crate) count: usize,
}
pub(crate) enum WatchInput {
    Native(OwnedHandle),
    #[cfg(test)]
    Simulated,
}
/// Private native boundary; fakes replace external effects, never registry policy.
pub(crate) trait NativePort: Send + Sync {
    fn create(
        &self,
        family: &mut Family,
        input: &mut WindowsLaunchInput,
    ) -> Result<Identity, RegistryError>;
    fn resume(&self, family: &mut Family) -> Result<(), RegistryError>;
    fn duplicate(&self, family: &mut Family, index: usize) -> Result<WatchInput, RegistryError>;
    fn enroll_desktop(
        &self,
        family: &mut Family,
        run: RunKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError>;
    fn seal(&self, family: &mut Family) -> Result<(), RegistryError>;
    fn inventory(&self, family: &mut Family) -> Result<Inventory, RegistryError>;
    fn terminate(&self, family: &mut Family) -> Result<(), RegistryError>;
    fn close(&self, family: &mut Family) -> Result<(), RegistryError>;
}
pub(crate) fn production() -> Arc<dyn NativePort> {
    Arc::new(WindowsPort)
}
struct WindowsPort;
pub(crate) fn handle(raw: Option<usize>, stage: &'static str) -> Result<HANDLE, RegistryError> {
    raw.map(|raw| HANDLE(raw as *mut _)).ok_or_else(|| RegistryError::invalid(stage))
}
pub(crate) fn close_slot(
    raw: &mut Option<usize>,
    stage: &'static str,
) -> Result<(), RegistryError> {
    if let Some(value) = *raw {
        // SAFETY: this slot uniquely owns a retained kernel handle, and failure preserves ownership.
        unsafe { CloseHandle(HANDLE(value as *mut _)) }
            .map_err(|cause| RegistryError::new(stage, cause))?;
        *raw = None;
    }
    Ok(())
}
mod cleanup;
mod creation;
mod inheritance;
mod lifecycle;
mod membership;
