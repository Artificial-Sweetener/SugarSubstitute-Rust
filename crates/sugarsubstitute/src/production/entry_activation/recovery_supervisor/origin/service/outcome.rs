// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow supervised failure custody without cloning panic strings or replacing completion funding.
use super::*;
/// The actual typed terminal and original MemoryLease remain one inline owner.
/// Diagnostics expose the terminal kind and closed unit/bool success value, never panic text.
/// Callers may inspect the retained original within this owner.
pub(super) struct RetainedOutcome<T> {
    pub(super) original: JobCompletion<T, OriginIssue>,
}
impl<T: std::fmt::Debug> std::fmt::Debug for RetainedOutcome<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut value = f.debug_struct("RetainedOriginOutcome");
        value.field("kind", &self.original.outcome.kind());
        if let JobOutcome::Success(result) = &self.original.outcome {
            value.field("success_value", result);
        }
        value.finish_non_exhaustive()
    }
}
impl<T> std::fmt::Display for RetainedOutcome<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Original recovery origin {:?} outcome retained", self.original.outcome.kind())
    }
}
impl<T: std::fmt::Debug + 'static> Error for RetainedOutcome<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.original.outcome {
            JobOutcome::Failure(cause) => Some(cause),
            _ => None,
        }
    }
}
