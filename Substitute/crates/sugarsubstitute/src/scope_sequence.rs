// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate monotone application scope identities on the native composition thread.
use std::{cell::Cell, rc::Rc};
use sugarsubstitute_contracts::execution::{IdentityError, ScopeId};
/// Share scope numbering across one process and its actual composed owners.
/// Numerical identities grant no execution/native capability. Rc keeps this
/// sequence on the same native thread as its real composing owners.
#[derive(Clone)]
pub(crate) struct ScopeSequence(Rc<Cell<u64>>);
impl ScopeSequence {
    /// Start the sole native-thread scope numbering shared by actual composed owners.
    pub(crate) fn new() -> Self {
        Self(Rc::new(Cell::new(0)))
    }
    /// Consume a positive identity before admission; rejection never reuses it.
    /// # Errors
    /// Exhaustion remains permanent across every same-thread alias.
    pub(crate) fn allocate(&self) -> Result<ScopeId, IdentityError> {
        let next = self.0.get().checked_add(1).ok_or(IdentityError::Exhausted)?;
        let id = ScopeId::new(next)?;
        self.0.set(next);
        Ok(id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child_and_parent_share_non_reused_monotone_scope_ids() -> Result<(), IdentityError> {
        let parent = ScopeSequence::new();
        let child = parent.clone();
        assert_eq!(parent.allocate()?.get(), 1);
        assert_eq!(child.allocate()?.get(), 2);
        assert_eq!(parent.allocate()?.get(), 3);
        assert_eq!(child.allocate()?.get(), 4);
        Ok(())
    }

    #[test]
    fn exhaustion_is_shared_and_cannot_reuse_a_scope_identity() -> Result<(), IdentityError> {
        let parent = ScopeSequence(Rc::new(Cell::new(u64::MAX - 1)));
        let child = parent.clone();
        assert_eq!(parent.allocate()?.get(), u64::MAX);
        assert!(matches!(child.allocate(), Err(IdentityError::Exhausted)));
        assert!(matches!(parent.allocate(), Err(IdentityError::Exhausted)));
        Ok(())
    }
}
