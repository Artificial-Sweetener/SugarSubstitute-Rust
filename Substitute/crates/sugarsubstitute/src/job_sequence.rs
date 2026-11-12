// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate application job identities across owners on their composition thread.

use std::{cell::Cell, rc::Rc};
use sugarsubstitute_contracts::execution::{IdentityError, JobId};

/// Share one nonwrapping sequence within one execution lifetime.
///
/// The UI or headless composition thread owns all clones. Allocate immediately
/// before admission; a rejected admission consumes its identity. Semantic frame
/// revisions remain independent. `Rc` prevents transfer to execution workers.
#[derive(Clone, Default)]
pub struct JobSequence(Rc<Cell<u64>>);

impl JobSequence {
    /// Begin the sole application sequence for a newly created execution owner.
    pub fn new() -> Self {
        Self(Rc::new(Cell::new(0)))
    }

    /// Allocate a positive identity, preserving exhaustion permanently.
    ///
    /// # Errors
    /// Returns `IdentityError::Exhausted` before wrapping; every clone stays exhausted.
    pub fn allocate(&self) -> Result<JobId, IdentityError> {
        let value = self.0.get().checked_add(1).ok_or(IdentityError::Exhausted)?;
        let identity = JobId::new(value)?;
        self.0.set(value);
        Ok(identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Interleaving root and rendering calls uses the same monotonically ordered identities.
    #[test]
    fn clones_share_allocation_and_rejected_identities_are_never_reused()
    -> Result<(), IdentityError> {
        let root = JobSequence::new();
        let render = root.clone();
        assert_eq!(root.allocate()?.get(), 1);
        assert_eq!(render.allocate()?.get(), 2);
        let rejected = root.allocate()?;
        assert_eq!(rejected.get(), 3);
        assert_eq!(render.allocate()?.get(), 4);
        Ok(())
    }

    /// Exhaustion remains closed across every existing clone.
    #[test]
    fn maximum_is_allocated_once_and_exhaustion_never_wraps() -> Result<(), IdentityError> {
        let root = JobSequence(Rc::new(Cell::new(u64::MAX - 1)));
        let render = root.clone();
        assert_eq!(render.allocate()?.get(), u64::MAX);
        assert_eq!(root.allocate(), Err(IdentityError::Exhausted));
        assert_eq!(render.allocate(), Err(IdentityError::Exhausted));
        Ok(())
    }
}
