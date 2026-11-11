// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve a pending first result across lazy controls and data destruction.

use super::{
    EntryPreparationInputError, EntryPreparationSuccess,
    outcome::{EntryPreparationCause, EntryPreparationFailure},
    owner::EntryPreparationOwner,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use sugarsubstitute_contracts::execution::JobOutcome;

pub(super) struct Pending<T, E> {
    value: Option<T>,
    failure: Option<EntryPreparationFailure<E>>,
}
impl<T, E> Pending<T, E> {
    pub(super) fn value(value: T) -> Self {
        Self { value: Some(value), failure: None }
    }

    pub(super) fn failure(failure: EntryPreparationFailure<E>) -> Self {
        Self { value: None, failure: Some(failure) }
    }

    pub(super) fn has_value(&self) -> bool {
        self.value.is_some()
    }

    pub(super) fn adopt_failure(mut self, mut failure: EntryPreparationFailure<E>) -> Self {
        // The failure owns its whole guard before successful data is destroyed.
        if let Some(value) = self.value.take()
            && let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(value)))
        {
            failure.append(EntryPreparationInputError::Control(
                super::EntryPreparationControlCause::ProviderPanic(crate::task::panic_cause(
                    payload,
                )),
            ));
        }
        self.failure = Some(failure);
        self
    }

    pub(super) fn observe(
        mut self,
        cause: EntryPreparationInputError,
        owner: &EntryPreparationOwner,
    ) -> Self {
        if let Some(failure) = self.failure.as_mut() {
            failure.append(cause);
            self
        } else {
            self.adopt_failure(EntryPreparationFailure::new(
                EntryPreparationCause::Preparation(cause),
                Some(owner.clone()),
            ))
        }
    }

    pub(super) fn finish(
        self,
        owner: EntryPreparationOwner,
    ) -> JobOutcome<EntryPreparationSuccess<T>, EntryPreparationFailure<E>> {
        if let Some(failure) = self.failure {
            JobOutcome::Failure(failure)
        } else if let Some(value) = self.value {
            JobOutcome::Success(EntryPreparationSuccess::new(value, owner))
        } else {
            JobOutcome::Failure(EntryPreparationFailure::new(
                EntryPreparationCause::Preparation(EntryPreparationInputError::Closed),
                Some(owner),
            ))
        }
    }
}
