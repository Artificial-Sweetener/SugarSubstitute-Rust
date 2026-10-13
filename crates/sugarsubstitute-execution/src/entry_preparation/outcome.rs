// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt first outcomes before observing later original controls.

use super::{EntryPreparationInputError, owner::EntryPreparationOwner};
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{MemoryClaim, PanicCause};

/// Preserve the first returned factory, control or actual callback-unwind cause.
pub enum EntryPreparationCause<E> {
    /// Original factory failure, unchanged by later cancellation or expiry.
    Factory(E),
    /// Actual original admission association or control failure.
    Preparation(EntryPreparationInputError),
    /// Actual bounded callback panic, retained under the original owner.
    Panicked(PanicCause),
    /// Actual capture failure remains owned even if the factory dropped its error.
    NativeEnvironment,
}
impl<E> fmt::Debug for EntryPreparationCause<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EntryPreparationCause { redacted }")
    }
}

/// Keep genuine first/later causes before their final original custody.
/// A typed failure can contain an actual panic; consumers inspect that cause
/// instead of inferring native settlement from terminal failure metadata.
pub struct EntryPreparationFailure<E> {
    cause: EntryPreparationCause<E>,
    secondary: [Option<EntryPreparationInputError>; 8],
    omitted: u64,
    _owner: Option<EntryPreparationOwner>,
    _unallocated: Option<MemoryClaim>,
}
impl<E> EntryPreparationFailure<E> {
    pub(super) fn new(
        cause: EntryPreparationCause<E>,
        owner: Option<EntryPreparationOwner>,
    ) -> Self {
        Self {
            cause,
            secondary: std::array::from_fn(|_| None),
            omitted: 0,
            _owner: owner,
            _unallocated: None,
        }
    }
    pub(super) fn unallocated(cause: EntryPreparationInputError, claim: MemoryClaim) -> Self {
        let mut failure = Self::new(EntryPreparationCause::Preparation(cause), None);
        failure._unallocated = Some(claim);
        failure
    }
    pub(super) fn append(&mut self, cause: EntryPreparationInputError) {
        if let Some(slot) = self.secondary.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
    /// Borrow the first actual source without separating its retaining owner.
    pub const fn cause(&self) -> &EntryPreparationCause<E> {
        &self.cause
    }
    /// Iterate the bounded later observations in their original order.
    pub fn secondary(&self) -> impl Iterator<Item = &EntryPreparationInputError> {
        self.secondary.iter().filter_map(Option::as_ref)
    }
    /// Count observations omitted from the eight retained secondary slots.
    pub const fn omitted_causes(&self) -> u64 {
        self.omitted
    }
    /// Borrow any genuine native cause retained by this same original owner.
    pub fn environment_cause(&self) -> Option<&super::cause::EntryEnvironmentCause> {
        self._owner.as_ref()?.payload().ok()?.native_failure.get().map(|failure| &failure.cause)
    }
}
impl<E> fmt::Debug for EntryPreparationFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EntryPreparationFailure { retained, redacted }")
    }
}
impl<E> fmt::Display for EntryPreparationFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original entry preparation failed")
    }
}
impl<E: Error + 'static> Error for EntryPreparationFailure<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.cause {
            EntryPreparationCause::Factory(cause) => Some(cause),
            EntryPreparationCause::Preparation(cause) => Some(cause),
            EntryPreparationCause::Panicked(_) => None,
            EntryPreparationCause::NativeEnvironment => {
                self.environment_cause().map(|cause| cause as &(dyn Error + 'static))
            }
        }
    }
}
