// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain successful data with its unchanged original whole preparation owner.

use super::{EntryPreparationInputError, EntryPreparationOwner, EntryRequestInput};
use std::fmt;

/// Keep successful prepared data before its original full-claim custody.
/// Borrowing grants no native privilege. Generic owned extraction is deliberately
/// absent; only checked concrete entry adoption transfers the complete aggregate.
pub struct EntryPreparationSuccess<T> {
    value: T,
    owner: EntryPreparationOwner,
}
impl<T> EntryPreparationSuccess<T> {
    /// Borrow prepared data while its original whole owner remains retained.
    pub const fn value(&self) -> &T {
        &self.value
    }

    pub(crate) fn new(value: T, owner: EntryPreparationOwner) -> Self {
        Self { value, owner }
    }

    pub(crate) fn owner(&self) -> &EntryPreparationOwner {
        &self.owner
    }
}
impl EntryPreparationSuccess<EntryRequestInput> {
    /// Move only the complete opaque input and its unchanged identical owner.
    /// The actual receiving execution client checks original publication first.
    /// Rejection keeps the complete original wrapper and its owned request.
    pub(crate) fn into_entry_request(
        self,
    ) -> Result<EntryRequestInput, (EntryPreparationInputError, Self)> {
        let Self { value, owner } = self;
        match value.adopt_owner(owner) {
            Ok(input) => Ok(input),
            Err((cause, value, owner)) => Err((cause, Self { value, owner })),
        }
    }
}
impl<T> fmt::Debug for EntryPreparationSuccess<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EntryPreparationSuccess { redacted }")
    }
}
