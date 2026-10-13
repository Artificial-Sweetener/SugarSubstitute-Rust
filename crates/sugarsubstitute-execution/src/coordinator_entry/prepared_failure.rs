// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse one bounded raw operation ledger while escaped aliases retain whole custody outside it.
use super::{EntryPreparedCause, EntryTransportError};
use crate::entry_preparation::EntryPreparationOwner;
use std::{
    fmt,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

pub(crate) struct PreparedFailurePayload {
    original: EntryPreparedCause,
    cleanup: [OnceLock<EntryPreparedCause>; 8],
    issued: AtomicU64,
    fatal: AtomicBool,
    owner_panic: OnceLock<sugarsubstitute_contracts::execution::PanicCause>,
}
pub(crate) struct ClosedPreparedSources(Option<Arc<PreparedFailurePayload>>);
impl ClosedPreparedSources {
    fn new(original: EntryPreparedCause) -> Self {
        Self(Some(Arc::new(PreparedFailurePayload {
            original,
            cleanup: std::array::from_fn(|_| OnceLock::new()),
            issued: AtomicU64::new(0),
            fatal: AtomicBool::new(false),
            owner_panic: OnceLock::new(),
        })))
    }
    fn payload(&self) -> Option<&PreparedFailurePayload> {
        self.0.as_deref()
    }
    fn same(&self, other: &Self) -> bool {
        matches!((&self.0,&other.0),(Some(a),Some(b)) if Arc::ptr_eq(a,b))
    }
}
impl Clone for ClosedPreparedSources {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for ClosedPreparedSources {
    fn drop(&mut self) {
        if let Some(source) = self.0.take()
            && let Some(source) = Arc::into_inner(source)
        {
            drop(source);
        }
    }
}
/// Retain one actual failed operation and eight later causes under its original owner.
/// Cached raw storage has no strong whole-owner backedge. Every escaped clone keeps
/// raw source storage first and unchanged original whole custody last. Repeated
/// failed observations reuse this ledger; they never allocate a second operation ledger.
pub struct EntryPreparedRetirement {
    sources: ClosedPreparedSources,
    owner: EntryPreparationOwner,
}
impl EntryPreparedRetirement {
    pub(crate) fn new(original: EntryTransportError, owner: EntryPreparationOwner) -> Self {
        owner.close_publication();
        let slot = owner.operation_sources();
        if let Some(sources) = slot.get() {
            let retained = Self { sources: sources.clone(), owner: owner.clone() };
            retained.retain(original);
            return retained;
        }
        let mut pending = Some(original);
        let sources = slot.get_or_init(|| {
            let original = pending.take().unwrap_or(EntryTransportError::Closed);
            ClosedPreparedSources::new(EntryPreparedCause::new(original, &owner))
        });
        let retained = Self { sources: sources.clone(), owner: owner.clone() };
        if let Some(cause) = pending {
            retained.retain(cause);
        }
        retained
    }
    /// Borrow actual primary through a nonclone source view.
    pub fn original(&self) -> Option<&EntryPreparedCause> {
        self.sources.payload().map(|payload| &payload.original)
    }
    /// Borrow at most eight actual later source views in their original slot order.
    pub fn cleanup_causes(&self) -> impl Iterator<Item = &EntryPreparedCause> {
        self.sources
            .payload()
            .into_iter()
            .flat_map(|payload| payload.cleanup.iter().filter_map(OnceLock::get))
    }
    /// Count actual later observations beyond the eight retained causes.
    pub fn omitted_cleanup(&self) -> u64 {
        self.sources
            .payload()
            .map_or(0, |payload| payload.issued.load(Ordering::Acquire).saturating_sub(8))
    }
    pub(crate) fn retain(&self, next: EntryTransportError) {
        if matches!(&next,EntryTransportError::PreparedRetirement(other) if self.sources.same(&other.sources))
        {
            return;
        }
        let Some(payload) = self.sources.payload() else {
            return;
        };
        if next.requires_process_shutdown() {
            payload.fatal.store(true, Ordering::Release);
        }
        let panic = match &next {
            EntryTransportError::Preparation(cause) => match cause.cause() {
                Some(crate::EntryPreparationInputError::Control(control)) => control.panic_cause(),
                _ => None,
            },
            other => other
                .native_cause()
                .and_then(crate::native::NativeObservationError::owner_panic_cause),
        };
        if let Some(panic) = panic {
            payload.owner_panic.get_or_init(|| panic.clone());
        }
        let mut index = payload.issued.load(Ordering::Acquire);
        loop {
            match payload.issued.compare_exchange_weak(
                index,
                index.saturating_add(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => index = actual,
            }
        }
        // Claim a fixed slot before allocating a converted source. Omitted causes
        // retain sticky facts but cannot allocate a tenth raw view/header.
        if index < 8
            && let Some(slot) = payload.cleanup.get(index as usize)
        {
            drop(slot.set(EntryPreparedCause::new(next, &self.owner)));
        }
    }
    pub(crate) fn contains_owner_panic(&self) -> bool {
        self.sources.payload().is_some_and(|payload| payload.owner_panic.get().is_some())
            || self.original().is_some_and(EntryPreparedCause::contains_panic)
            || self.cleanup_causes().any(EntryPreparedCause::contains_panic)
    }
    pub(crate) fn requires_process_shutdown(&self) -> bool {
        self.sources.payload().is_some_and(|payload| payload.fatal.load(Ordering::Acquire))
            || self.original().is_some_and(EntryPreparedCause::fatal)
            || self.cleanup_causes().any(EntryPreparedCause::fatal)
    }
}
impl Clone for EntryPreparedRetirement {
    fn clone(&self) -> Self {
        Self { sources: self.sources.clone(), owner: self.owner.clone() }
    }
}
impl fmt::Debug for EntryPreparedRetirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EntryPreparedRetirement { retained, redacted }")
    }
}
impl EntryTransportError {
    pub(crate) fn prepared(
        original: Self,
        after: Option<Self>,
        owner: Option<&EntryPreparationOwner>,
    ) -> Self {
        let Some(owner) = owner else {
            let mut original = original;
            if let Some(after) = after {
                original.retain_cleanup(after);
            }
            return original;
        };
        let retained = EntryPreparedRetirement::new(original, owner.clone());
        if let Some(after) = after {
            retained.retain(after);
        }
        Self::PreparedRetirement(retained)
    }
}
