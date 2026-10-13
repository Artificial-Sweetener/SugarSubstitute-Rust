// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow actual primary causes without projecting their cloneable retained wrapper.
use super::EntryTransportError;
use crate::{
    entry_preparation::{
        EntryPreparationInputError, EntryPreparationOwner, error::PreparationSource,
    },
    native::NativeObservationError,
};
use std::{error::Error, fmt};

/// Inert typed classification of one retained cause; it grants no native privileges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryPreparedCauseKind {
    /// The original session or connection was closing.
    Closed,
    /// A bounded slot remained occupied.
    Busy,
    /// The operation identity was invalid or reused.
    InvalidOperation,
    /// The original absolute deadline was invalid.
    InvalidDeadline,
    /// The current preparation or session phase rejected the effect.
    InvalidPhase,
    /// Actual peer or correlation validation failed.
    Authentication,
    /// The bounded frame schema or shape was invalid.
    Malformed,
    /// The authoritative Domain request validator rejected its shape.
    Shape,
    /// The genuine original preparation control rejected publication.
    Preparation,
    /// A fixed inventory or bounded payload was full.
    Capacity,
    /// The original clock or counter observation failed.
    Clock,
    /// The original native boundary failed.
    Native,
    /// Original failure retained a separate actual cleanup failure.
    Retirement,
}
/// Borrow genuine source data without an owned Arc or original-custody projection.
pub struct EntryPreparedCause(RawCause);
enum RawCause {
    Transport(Box<EntryTransportError>),
    Preparation(PreparationSource),
    Retirement {
        original: Box<EntryPreparedCause>,
        cleanup: NativeObservationError,
        identity_unrestored: bool,
    },
}
impl EntryPreparedCause {
    pub(super) fn new(error: EntryTransportError, owner: &EntryPreparationOwner) -> Self {
        match error {
            EntryTransportError::Preparation(error) => {
                let (source, original) = error.into_source();
                if original.same_owner(owner) {
                    // Original remains alive until raw source has entered this owned result.
                    let retained = Self(RawCause::Preparation(source));
                    drop(original);
                    retained
                } else {
                    let retained = Self::foreign(owner);
                    drop(source);
                    drop(original);
                    retained
                }
            }
            EntryTransportError::PreparedRetirement(other) => {
                // A current ledger's own clone is rejected before this conversion.
                // A foreign operation is not an actual current cleanup observation.
                let retained = Self::foreign(owner);
                drop(other);
                retained
            }
            EntryTransportError::Retirement { original, cleanup, identity_unrestored } => {
                Self(RawCause::Retirement {
                    original: Box::new(Self::new(*original, owner)),
                    cleanup,
                    identity_unrestored,
                })
            }
            other => Self(RawCause::Transport(Box::new(other))),
        }
    }
    fn foreign(owner: &EntryPreparationOwner) -> Self {
        let error = crate::EntryPreparationError::new(
            EntryPreparationInputError::ForeignOwner,
            owner.clone(),
        );
        let (source, original) = error.into_source();
        let retained = Self(RawCause::Preparation(source));
        drop(original);
        retained
    }
    /// Return inert classification without returning a cloneable retained error.
    pub fn kind(&self) -> EntryPreparedCauseKind {
        match &self.0 {
            RawCause::Preparation(_) => EntryPreparedCauseKind::Preparation,
            RawCause::Retirement { .. } => EntryPreparedCauseKind::Retirement,
            RawCause::Transport(error) => match error.as_ref() {
                EntryTransportError::Closed => EntryPreparedCauseKind::Closed,
                EntryTransportError::Busy => EntryPreparedCauseKind::Busy,
                EntryTransportError::InvalidOperation => EntryPreparedCauseKind::InvalidOperation,
                EntryTransportError::InvalidDeadline => EntryPreparedCauseKind::InvalidDeadline,
                EntryTransportError::InvalidPhase => EntryPreparedCauseKind::InvalidPhase,
                EntryTransportError::Authentication => EntryPreparedCauseKind::Authentication,
                EntryTransportError::Malformed => EntryPreparedCauseKind::Malformed,
                EntryTransportError::Shape(_) => EntryPreparedCauseKind::Shape,
                EntryTransportError::Capacity => EntryPreparedCauseKind::Capacity,
                EntryTransportError::Clock(_) => EntryPreparedCauseKind::Clock,
                EntryTransportError::Native { .. } => EntryPreparedCauseKind::Native,
                _ => EntryPreparedCauseKind::Retirement,
            },
        }
    }
    /// Borrow nonclone actual original preparation control/error data.
    pub fn preparation_cause(&self) -> Option<&EntryPreparationInputError> {
        match &self.0 {
            RawCause::Preparation(source) => source.cause(),
            _ => None,
        }
    }
    /// Borrow the fixed native stage label without projecting native cause ownership.
    pub fn native_stage(&self) -> Option<&'static str> {
        self.native().map(NativeObservationError::stage)
    }
    fn native(&self) -> Option<&NativeObservationError> {
        match &self.0 {
            RawCause::Transport(error) => error.native_cause(),
            RawCause::Retirement { original, cleanup, .. } => original.native().or(Some(cleanup)),
            RawCause::Preparation(_) => None,
        }
    }
    pub(super) fn panic(&self) -> Option<&sugarsubstitute_contracts::execution::PanicCause> {
        match &self.0 {
            RawCause::Preparation(source) => match source.cause()? {
                EntryPreparationInputError::Control(control) => control.panic_cause(),
                _ => None,
            },
            RawCause::Retirement { original, cleanup, .. } => {
                original.panic().or_else(|| cleanup.owner_panic_cause())
            }
            RawCause::Transport(error) => {
                error.native_cause().and_then(NativeObservationError::owner_panic_cause)
            }
        }
    }
    pub(super) fn fatal(&self) -> bool {
        match &self.0 {
            RawCause::Transport(error) => error.requires_process_shutdown(),
            RawCause::Retirement { original, identity_unrestored, .. } => {
                *identity_unrestored || original.fatal()
            }
            RawCause::Preparation(_) => false,
        }
    }
    pub(super) fn contains_panic(&self) -> bool {
        self.panic().is_some()
            || self.native().is_some_and(NativeObservationError::contains_owner_panic)
    }
}
impl fmt::Debug for EntryPreparedCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryPreparedCause").field("kind", &self.kind()).finish_non_exhaustive()
    }
}
impl fmt::Display for EntryPreparedCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Retained original entry cause")
    }
}
impl Error for EntryPreparedCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.0 {
            RawCause::Preparation(source) => {
                source.cause().map(|cause| cause as &(dyn Error + 'static))
            }
            RawCause::Retirement { original, .. } => Some(original.as_ref()),
            // Preserve the genuine underlying primary; the cloneable native wrapper stays private.
            RawCause::Transport(error) => match error.as_ref() {
                EntryTransportError::Clock(cause) | EntryTransportError::Native { cause, .. } => {
                    cause.source()
                }
                other => other.source(),
            },
        }
    }
}
