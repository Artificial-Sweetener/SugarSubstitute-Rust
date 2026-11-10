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
use sugarsubstitute_contracts::incident::DiagnosticKey;
use sugarsubstitute_contracts::lifecycle::{ProcessKey, RunKey};
use sugarsubstitute_execution::windows_diagnostic_pipe::{
    DiagnosticEnrollment, DiagnosticPipeBootstrap,
};
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
    pub(crate) preparation_image:
        Option<Box<crate::registry_input::preparation::PreparationLaunch>>,
    pub(crate) job: Option<usize>,
    pub(crate) rejected_job: Option<usize>,
    pub(crate) cache_family: Option<crate::setup_storage::cache_custody::CacheFamilyId>,
    pub(crate) thread: Option<usize>,
    pub(crate) watch_duplicate: Option<usize>,
    pub(crate) members: [Option<Member>; MAX_MEMBERS],
    pub(crate) count: usize,
    pub(crate) root: Option<Identity>,
    pub(crate) attributes: Vec<usize>,
    pub(crate) query: Vec<usize>,
    pub(crate) attributes_initialized: bool,
    pub(crate) job_list: [usize; 1],
    pub(crate) recovery_carrier:
        Option<Box<crate::registry_input::recovery::RecoveryCarrierLaunch>>,
    pub(crate) fixture_events: Option<FixtureChildEvents>,
    pub(crate) lifecycle_bootstrap: Option<LifecyclePipeBootstrap>,
    pub(crate) lifecycle_enrollment: Option<LifecycleDesktopEnrollment>,
    pub(crate) lifecycle_run: Option<RunKey>,
    pub(crate) diagnostic_bootstrap: Option<DiagnosticPipeBootstrap>,
    pub(crate) diagnostic_enrollment: Option<DiagnosticEnrollment>,
    pub(crate) diagnostic_key: Option<DiagnosticKey>,
    pub(crate) backend_custody: Option<crate::registry_input::backend_custody::CustodyLaunch>,
    pub(crate) output_pin: Option<OutputPin>,
    pub(crate) output_cancel: Option<OutputCancellation>,
    pub(crate) output_identity: Option<sugarsubstitute_execution::windows_output::OutputIdentity>,
    pub(crate) inherited_handles: [usize; 6],
    pub(crate) inherited_count: usize,
    pub(crate) recovery_stdio: bool,
    pub(crate) stdio_handles: [usize; 3],
    pub(crate) sealed: bool,
    pub(crate) incomplete: bool,
    pub(crate) forced: bool,
    pub(crate) resumed: bool,
    pub(crate) closure_committed: bool,
    custody: Option<crate::native_claims::backend_custody::pool::WitnessSlot>,
    creation_memory: Option<sugarsubstitute_execution::process_creation::ProcessCreationMemory>,
}
/// Keep the complete native allocation before its unchanged original family claim.
/// Custody families instead retain their already-funded slot outside this owner.
pub(crate) struct FamilyStorage {
    family: Box<Family>,
    _memory: Option<MemoryClaim>,
    _creation_memory: Option<sugarsubstitute_execution::process_creation::ProcessCreationMemory>,
}
impl std::ops::Deref for FamilyStorage {
    type Target = Family;
    fn deref(&self) -> &Family {
        &self.family
    }
}
impl std::ops::DerefMut for FamilyStorage {
    fn deref_mut(&mut self) -> &mut Family {
        &mut self.family
    }
}
impl Family {
    /// Claim the original fixed family budget before allocating bounded native storage.
    pub(crate) fn allocate_owned(memory: &MemoryLease) -> Result<FamilyStorage, RegistryError> {
        let claim = memory
            .claim(crate::registry_input::FAMILY_STORAGE_BYTES)
            .map_err(|cause| RegistryError::new("family_memory", cause))?;
        Self::allocate(Some(claim))
    }
    /// Allocate the one native Family only under an existing original slot guard.
    fn allocate(claim: Option<MemoryClaim>) -> Result<FamilyStorage, RegistryError> {
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
            + crate::registry_input::preparation::PreparationLaunch::storage_bytes()
            + crate::registry::family_shared_storage_bytes()
            + (attributes.capacity() + query.capacity()) * std::mem::size_of::<usize>();
        if actual as u64 > crate::registry_input::FAMILY_STORAGE_BYTES {
            return Err(RegistryError::invalid("family_capacity"));
        }
        Ok(FamilyStorage {
            family: Box::new(Self {
                preparation_image: None,
                job: None,
                rejected_job: None,
                cache_family: None,
                thread: None,
                watch_duplicate: None,
                members: std::array::from_fn(|_| None),
                count: 0,
                root: None,
                attributes,
                query,
                attributes_initialized: false,
                job_list: [0],
                recovery_carrier: None,
                fixture_events: None,
                lifecycle_bootstrap: None,
                lifecycle_enrollment: None,
                lifecycle_run: None,
                diagnostic_bootstrap: None,
                diagnostic_enrollment: None,
                diagnostic_key: None,
                backend_custody: None,
                output_pin: None,
                output_cancel: None,
                output_identity: None,
                inherited_handles: [0; 6],
                inherited_count: 0,
                recovery_stdio: false,
                stdio_handles: [0; 3],
                sealed: false,
                incomplete: false,
                forced: false,
                resumed: false,
                closure_committed: false,
                custody: None,
                creation_memory: None,
            }),
            _memory: claim,
            _creation_memory: None,
        })
    }
    /// Allocate the identical native Family under its already-admitted creation claim.
    pub(crate) fn custodied(
        memory: &sugarsubstitute_execution::process_creation::ProcessCreationMemory,
        consumer_bytes: usize,
    ) -> Result<FamilyStorage, RegistryError> {
        if Self::storage_bytes() + consumer_bytes > memory.bytes() as usize {
            return Err(RegistryError::invalid("custodied_family_capacity"));
        }
        let mut family = Self::allocate(None)?;
        family.creation_memory = Some(memory.clone());
        family._creation_memory = Some(memory.clone());
        Ok(family)
    }
    /// Count native Family storage before the same original fixed allocation.
    pub(crate) const fn storage_bytes() -> usize {
        size_of::<Self>()+crate::registry_input::preparation::PreparationLaunch::storage_bytes()+2048*size_of::<usize>()+(MAX_MEMBERS+2)*size_of::<usize>()+size_of::<FamilyStorage>()
            + sugarsubstitute_execution::process_creation::ProcessCreationMemory::control_storage_bytes()
    }
    /// End fresh creation checks only after actual resumed-family ownership transfers.
    pub(crate) fn creation_transferred(&mut self) {
        self.creation_memory = None;
    }
    /// Gate custody effects with the original slot; existing owners keep their control.
    fn check_native(&self) -> Result<(), RegistryError> {
        if let Some(memory) = &self.creation_memory {
            memory
                .check_effect()
                .map_err(|cause| RegistryError::new("custodied_family_control", cause))?;
        }
        self.custody.as_ref().map_or(Ok(()), |slot| {
            slot.check().map_err(|cause| RegistryError::new("custody_family_control", cause))
        })
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
    /// Prepare one actual empty Job before the acknowledged suspended root.
    fn prepare_job(&self, family: &mut Family) -> Result<(), RegistryError>;
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
    fn enroll_diagnostic(
        &self,
        family: &mut Family,
        binding: DiagnosticKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError>;
    fn enroll_backend_custody(
        &self,
        family: &mut Family,
        binding: sugarsubstitute_contracts::backend_custody::BackendCustodyKey,
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
mod backend_custody;
mod cleanup;
mod creation;
pub(crate) mod custody;
pub(crate) mod custody_transfer;
mod diagnostic;
mod inheritance;
mod lifecycle;
mod membership;
mod recovery_carrier;

mod recovery_child;
pub(crate) use recovery_child::close_recovery_input;

#[cfg(test)]
pub(crate) mod cache_fixture;
#[cfg(test)]
mod creation_tests;
