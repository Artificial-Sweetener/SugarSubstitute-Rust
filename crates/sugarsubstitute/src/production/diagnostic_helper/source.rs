// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore the identical partial operation on unwinding without holding a lock at providers.
use super::{SeedInput, SeedOperation, service::SeedServiceIssue};
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::{
    JobContext, MemoryClaim, MemoryClaimError, MemoryLease,
};

pub(in crate::production) const SEED_SOURCE_BYTES: u64 = 16 * 1024;
struct State {
    input: Option<SeedInput>,
    operation: Option<Box<SeedOperation>>,
    started: bool,
}
struct Shared {
    state: Mutex<State>,
    owner: sugarsubstitute_contracts::execution::ExecutionId,
    _claim: MemoryClaim,
}
/// The outside reservation outlives destruction of the final Arc control allocation.
pub(super) struct Source {
    shared: Arc<Shared>,
    _outside: MemoryLease,
}
const _: () = assert!(
    size_of::<Shared>()
        + size_of::<SeedOperation>()
        + 2 * size_of::<usize>()
        + size_of::<super::service::SeedService>()
        < SEED_SOURCE_BYTES as usize
);
impl Source {
    pub(super) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        let claim = context.memory().claim(SEED_SOURCE_BYTES)?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                input: None,
                operation: Some(Box::new(SeedOperation::new())),
                started: false,
            }),
            owner: context.owner_id(),
            _claim: claim,
        });
        Ok(Self { shared, _outside: context.memory().clone() })
    }
    pub(super) fn owner(&self) -> sugarsubstitute_contracts::execution::ExecutionId {
        self.shared.owner
    }
    pub(super) fn alias(&self) -> Self {
        Self { shared: self.shared.clone(), _outside: self._outside.clone() }
    }
    pub(super) fn install(&self, input: &mut Option<SeedInput>) -> Result<(), SeedServiceIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.started || state.input.is_some() {
            return Err(SeedServiceIssue::Started);
        }
        if state.operation.is_none() {
            state.operation = Some(Box::new(SeedOperation::new()));
        }
        state.input = Some(input.take().ok_or(SeedServiceIssue::Identity)?);
        Ok(())
    }
    /// Release actual closed records outside the source lock after domain adoption.
    pub(super) fn release_retired(&self) -> Result<(), SeedServiceIssue> {
        let operation = {
            let mut state =
                self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let operation = state.operation.as_ref().ok_or(SeedServiceIssue::Busy)?;
            if !state.started
                || !operation.files_retired()
                || operation.failure().is_some()
                || operation.memory_failure().is_some()
                || operation.retirement_failure().is_some()
                || operation.committed().is_none()
            {
                return Err(SeedServiceIssue::Busy);
            }
            let operation = state.operation.take();
            state.started = false;
            operation
        };
        drop(operation);
        Ok(())
    }
    /// Compact only checked-closed failed records after moving passive causes to their service.
    /// Retained input from rejected admission has performed no native effects. Drop outside the lock.
    pub(super) fn release_failed_retired(&self) -> Result<(), SeedServiceIssue> {
        let (operation, input) = {
            let mut state =
                self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.operation.as_ref().is_none_or(|operation| !operation.files_retired()) {
                return Err(SeedServiceIssue::Busy);
            }
            let operation = state.operation.take();
            let input = state.input.take();
            state.started = false;
            (operation, input)
        };
        drop(operation);
        drop(input);
        Ok(())
    }
    pub(super) fn begin(&self) -> Result<(Loan, SeedInput), SeedServiceIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.started {
            return Err(SeedServiceIssue::Started);
        }
        if state.operation.is_none() {
            return Err(SeedServiceIssue::Busy);
        }
        let input = state.input.take().ok_or(SeedServiceIssue::Identity)?;
        let operation = state.operation.take().ok_or(SeedServiceIssue::Busy)?;
        state.started = true;
        Ok((Loan { source: self.alias(), operation: Some(operation) }, input))
    }
    pub(super) fn loan(&self) -> Result<Loan, SeedServiceIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let operation = state.operation.take().ok_or(SeedServiceIssue::Busy)?;
        Ok(Loan { source: self.alias(), operation: Some(operation) })
    }
    /// Observe only passive fields; never invoke a provider while the source lock is held.
    pub(super) fn inspect<T>(&self, observe: impl FnOnce(&SeedOperation) -> T) -> Option<T> {
        let state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation.as_deref().map(observe)
    }
    /// Move the actual passive cause once; original native/data custody remains in this cell.
    pub(super) fn take_failure(&self) -> Option<super::SeedFailure> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let operation = state.operation.as_deref_mut()?;
        if !operation.files_retired() {
            return None;
        }
        super::SeedFailure::take(operation)
    }
}
pub(super) struct Loan {
    source: Source,
    operation: Option<Box<SeedOperation>>,
}
impl Loan {
    pub(super) fn operation(&mut self) -> Result<&mut SeedOperation, SeedServiceIssue> {
        self.operation.as_deref_mut().ok_or(SeedServiceIssue::Busy)
    }
}
impl Drop for Loan {
    fn drop(&mut self) {
        let mut state =
            self.source.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // Only taking the unique occupied cell creates a loan; no effect runs under this lock.
        state.operation = self.operation.take();
    }
}
