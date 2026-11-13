// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep bounded raw source storage first and its unchanged whole owner last.
use super::{
    EntryPreparationControlCause, EntryPreparationInputError, owner::EntryPreparationOwner,
};
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::execution::TimeError;

pub(crate) struct FailurePayload {
    cause: EntryPreparationInputError,
}
pub(crate) enum PreparationSource {
    Inline { cause: EntryPreparationInputError, kind: InlineKind },
    Shared(Option<Arc<FailurePayload>>),
}
#[derive(Clone, Copy)]
pub(crate) enum InlineKind {
    Closed,
    ForeignOwner,
    OriginalSetupMismatch,
    Cancelled,
    Deadline,
    Clock(TimeError),
}
impl InlineKind {
    fn cause(self) -> EntryPreparationInputError {
        match self {
            Self::Closed => EntryPreparationInputError::Closed,
            Self::ForeignOwner => EntryPreparationInputError::ForeignOwner,
            Self::OriginalSetupMismatch => EntryPreparationInputError::OriginalSetupMismatch,
            Self::Cancelled => {
                EntryPreparationInputError::Control(EntryPreparationControlCause::Cancelled)
            }
            Self::Deadline => {
                EntryPreparationInputError::Control(EntryPreparationControlCause::Deadline)
            }
            Self::Clock(cause) => {
                EntryPreparationInputError::Control(EntryPreparationControlCause::Clock(cause))
            }
        }
    }
    fn from(cause: &EntryPreparationInputError) -> Option<Self> {
        match cause {
            EntryPreparationInputError::Closed => Some(Self::Closed),
            EntryPreparationInputError::ForeignOwner => Some(Self::ForeignOwner),
            EntryPreparationInputError::OriginalSetupMismatch => Some(Self::OriginalSetupMismatch),
            EntryPreparationInputError::Control(EntryPreparationControlCause::Cancelled) => {
                Some(Self::Cancelled)
            }
            EntryPreparationInputError::Control(EntryPreparationControlCause::Deadline) => {
                Some(Self::Deadline)
            }
            EntryPreparationInputError::Control(EntryPreparationControlCause::Clock(cause)) => {
                Some(Self::Clock(*cause))
            }
            _ => None,
        }
    }
}
impl PreparationSource {
    pub(super) fn shared(cause: EntryPreparationInputError) -> Self {
        Self::Shared(Some(Arc::new(FailurePayload { cause })))
    }
    pub(crate) fn cause(&self) -> Option<&EntryPreparationInputError> {
        match self {
            Self::Inline { cause, .. } => Some(cause),
            Self::Shared(source) => source.as_ref().map(|source| &source.cause),
        }
    }
}
impl Clone for PreparationSource {
    fn clone(&self) -> Self {
        match self {
            Self::Inline { kind, .. } => Self::Inline { cause: kind.cause(), kind: *kind },
            Self::Shared(source) => Self::Shared(source.clone()),
        }
    }
}
impl Drop for PreparationSource {
    fn drop(&mut self) {
        if let Self::Shared(source) = self
            && let Some(source) = source.take()
            && let Some(source) = Arc::into_inner(source)
        {
            drop(source);
        }
    }
}
/// Retain genuine bounded original failures without exposing source/custody separation.
/// Primitive controls allocate nothing. Dynamic observations keep first plus eight
/// later raw sources in the original owner; overflow borrows the first retained source
/// and increments its observable omission count. Providers still run at every boundary.
pub struct EntryPreparationError {
    source: PreparationSource,
    owner: EntryPreparationOwner,
}
impl EntryPreparationError {
    pub(crate) fn new(cause: EntryPreparationInputError, owner: EntryPreparationOwner) -> Self {
        if let Some(kind) = InlineKind::from(&cause) {
            return Self { source: PreparationSource::Inline { cause, kind }, owner };
        }
        let (source, pending) = match owner.payload() {
            Ok(payload) => payload.sources.retain(cause),
            Err(_) => (None, Some(cause)),
        };
        let retained = Self {
            source: source.unwrap_or_else(|| PreparationSource::Inline {
                cause: EntryPreparationInputError::Closed,
                kind: InlineKind::Closed,
            }),
            owner,
        };
        drop(pending);
        retained
    }
    #[cfg(any(windows, test))]
    pub(crate) fn retained_owner(&self) -> Option<&EntryPreparationOwner> {
        Some(&self.owner)
    }
    #[cfg(any(windows, test))]
    pub(crate) fn into_source(self) -> (PreparationSource, EntryPreparationOwner) {
        let Self { source, owner } = self;
        (source, owner)
    }
    /// Borrow the genuine retained primary source; formatting remains redacted.
    pub fn cause(&self) -> Option<&EntryPreparationInputError> {
        self.source.cause()
    }
    /// Count genuine dynamic source observations beyond first plus eight retained slots.
    pub fn omitted_observations(&self) -> u64 {
        self.owner.payload().map_or(0, |payload| payload.sources.omitted())
    }
}
impl Clone for EntryPreparationError {
    fn clone(&self) -> Self {
        Self { source: self.source.clone(), owner: self.owner.clone() }
    }
}
impl fmt::Debug for EntryPreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EntryPreparationError { retained, redacted }")
    }
}
impl fmt::Display for EntryPreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original entry preparation validation failed")
    }
}
impl Error for EntryPreparationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause().map(|cause| cause as &(dyn Error + 'static))
    }
}
