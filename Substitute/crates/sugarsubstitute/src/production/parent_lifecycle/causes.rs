// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move the same eight original backend causes and their charged allocation between retired and fresh roles.
use super::{ParentError, ParentLifecycle, attempt::SettledAttemptFailure};
use std::{collections::TryReserveError, error::Error, fmt};
use sugarsubstitute_contracts::{execution::MemoryClaim, lifecycle::ResourceKey};
const SLOTS: usize = 8;
/// Preserve fallible allocation causes before constructing a fresh owner; no native fact is implied.
#[derive(Debug)]
pub(in crate::production) enum CauseStorageError {
    Storage,
    Allocation(TryReserveError),
}
impl fmt::Display for CauseStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Backend cause storage rejected")
    }
}
impl Error for CauseStorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Allocation(cause) => Some(cause),
            Self::Storage => None,
        }
    }
}
/// One fixed-capacity allocation, carrying its ORIGINAL whole Parent two-MiB claim.
/// Parent stores this owner last, so all actor data and this heap die before the claim.
/// A retired role moves it through the replacement Record; inherited construction never allocates a second slot array.
pub(in crate::production) struct BackendCauses {
    slots: Box<[Option<SettledAttemptFailure>]>,
    storage: MemoryClaim,
}
impl BackendCauses {
    /// Allocate only under an already admitted fresh original Parent partition.
    pub(in crate::production) fn new(
        storage: MemoryClaim,
    ) -> std::result::Result<Self, CauseStorageError> {
        if storage.bytes() != ParentLifecycle::storage_bytes() {
            return Err(CauseStorageError::Storage);
        }
        let mut slots = Vec::new();
        slots.try_reserve_exact(SLOTS).map_err(CauseStorageError::Allocation)?;
        if slots.capacity() != SLOTS {
            return Err(CauseStorageError::Storage);
        }
        slots.resize_with(SLOTS, || None);
        Ok(Self { slots: slots.into_boxed_slice(), storage })
    }
    /// Heap bytes enter the unchanged Parent partition before any role effect.
    pub(in crate::production) const fn heap_bytes() -> u64 {
        (SLOTS * size_of::<Option<SettledAttemptFailure>>()) as u64
    }
    pub(super) fn storage_bytes(&self) -> u64 {
        self.storage.bytes()
    }
    #[cfg(test)]
    pub(super) fn first(&self) -> Option<&SettledAttemptFailure> {
        self.slots.iter().flatten().next()
    }
    #[cfg(test)]
    pub(super) fn full(&self) -> bool {
        self.slots.iter().all(Option::is_some)
    }
    pub(super) fn contains(&self, key: ResourceKey) -> bool {
        self.slots.iter().flatten().any(|value| value.resource == key)
    }
    pub(super) fn free_slot(&mut self) -> Option<&mut Option<SettledAttemptFailure>> {
        self.slots.iter_mut().find(|slot| slot.is_none())
    }
    /// Move the same live heap and claim before its old container drops; rejection preserves both owners.
    /// This data move grants no native retirement authority. The Parent extraction path checks its actual proof first.
    pub(in crate::production) fn transfer(
        from: &mut Option<Self>,
        to: &mut Option<Self>,
    ) -> std::result::Result<(), ParentError> {
        if to.is_some() {
            return Err(ParentError::Overloaded);
        }
        *to = from.take();
        Ok(())
    }
    #[cfg(test)]
    pub(in crate::production) fn insert_for_owner_test(
        &mut self,
        value: SettledAttemptFailure,
    ) -> std::result::Result<(), ParentError> {
        *self.free_slot().ok_or(ParentError::Overloaded)? = Some(value);
        Ok(())
    }
    #[cfg(test)]
    pub(in crate::production) fn allocation(&self) -> *const Option<SettledAttemptFailure> {
        self.slots.as_ptr()
    }
    #[cfg(test)]
    pub(in crate::production) fn claim(&self) -> &MemoryClaim {
        &self.storage
    }
}
impl ParentLifecycle {
    /// Extract original charged cause custody only after this exact Parent's real native and policy retirement.
    /// Failure preserves the owner; no copied Closed/exit/EOF state substitutes for the checked proof.
    pub(in crate::production) fn take_retired_backend_causes(
        &mut self,
    ) -> std::result::Result<Option<BackendCauses>, ParentError> {
        if !self.closed || !self.native_retired() || !self.settled() {
            return Err(ParentError::WrongPhase);
        }
        Ok(self.backend_causes.take())
    }
}

#[cfg(test)]
impl ParentLifecycle {
    pub(in crate::production) fn cause_allocation_for_owner_test(
        &self,
    ) -> std::result::Result<*const Option<SettledAttemptFailure>, ParentError> {
        Ok(self.backend_causes.as_ref().ok_or(ParentError::WrongPhase)?.allocation())
    }
    pub(in crate::production) fn cause_reserved_for_owner_test(
        &self,
    ) -> std::result::Result<u64, ParentError> {
        Ok(self.backend_causes.as_ref().ok_or(ParentError::WrongPhase)?.claim().reserved_bytes())
    }
}

#[cfg(test)]
#[path = "causes/tests.rs"]
mod tests;
