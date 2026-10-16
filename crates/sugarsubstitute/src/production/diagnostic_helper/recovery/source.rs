// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep partial original recovery custody across worker failure or unwind.
use super::*;
use std::sync::{Arc, Mutex};
struct State {
    input: Option<RecoveryInput>,
    operation: Option<Box<RecoveryOperation>>,
    started: bool,
}
struct Shared {
    state: Mutex<State>,
    owner: ExecutionId,
    _claim: MemoryClaim,
}
pub(super) struct Source {
    shared: Arc<Shared>,
    _outside: MemoryLease,
}
const _: () = assert!(
    size_of::<Shared>()
        + size_of::<RecoveryOperation>()
        + size_of::<RecoveryService>()
        + 4 * size_of::<usize>()
        < (RECOVERY_SOURCE_BYTES - HistoricalCaptureRecovery::required_storage_bytes()) as usize
);
impl Source {
    pub(super) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        let claim = context
            .memory()
            .claim(RECOVERY_SOURCE_BYTES - HistoricalCaptureRecovery::required_storage_bytes())?;
        let historical =
            context.memory().claim(HistoricalCaptureRecovery::required_storage_bytes())?;
        let mut operation = RecoveryOperation::new();
        // The exact fixed claim was checked above; constructor performs no native effect.
        operation.historical = Some(HistoricalCaptureRecovery::new(historical)?);
        Ok(Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    input: None,
                    operation: Some(Box::new(operation)),
                    started: false,
                }),
                owner: context.owner_id(),
                _claim: claim,
            }),
            _outside: context.memory().clone(),
        })
    }
    pub(super) fn owner(&self) -> ExecutionId {
        self.shared.owner
    }
    pub(super) fn alias(&self) -> Self {
        Self { shared: self.shared.clone(), _outside: self._outside.clone() }
    }
    pub(super) fn install(&self, input: &mut Option<RecoveryInput>) -> Result<(), RecoveryIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.started || state.input.is_some() || state.operation.is_none() {
            return Err(RecoveryIssue::Started);
        }
        state.input = Some(input.take().ok_or(RecoveryIssue::Identity)?);
        Ok(())
    }
    /// Restore only the identical never-started request after genuine central no-admission.
    pub(super) fn restore_unstarted(
        &self,
        output: &mut Option<RecoveryInput>,
    ) -> Result<(), RecoveryIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if output.is_some()
            || state.started
            || state.operation.as_ref().is_none_or(|operation| {
                operation.repository.is_some()
                    || operation.pending.is_some()
                    || operation.input.is_some()
            })
        {
            return Err(RecoveryIssue::Started);
        }
        *output = state.input.take();
        Ok(())
    }
    pub(super) fn begin(&self) -> Result<Loan, RecoveryIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.started {
            return Err(RecoveryIssue::Started);
        }
        if state.input.is_none() {
            return Err(RecoveryIssue::Identity);
        }
        let mut operation = state.operation.take().ok_or(RecoveryIssue::Busy)?;
        operation.input = state.input.take();
        state.started = true;
        Ok(Loan { source: self.alias(), operation: Some(operation) })
    }
    /// Re-arm only after the service observed actual terminal and checked native retirement.
    /// The same funded operation persists; input remains with the caller on rejection.
    pub(super) fn advance(&self, input: &mut Option<RecoveryInput>) -> Result<(), RecoveryIssue> {
        let mut loan = self.loan()?;
        loan.operation()?.rearm(input)?;
        let next = loan.operation()?.input.take().ok_or(RecoveryIssue::Identity)?;
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.started = false;
        state.input = Some(next);
        Ok(())
    }
    pub(super) fn loan(&self) -> Result<Loan, RecoveryIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let operation = state.operation.take().ok_or(RecoveryIssue::Busy)?;
        Ok(Loan { source: self.alias(), operation: Some(operation) })
    }
    pub(super) fn inspect<T>(&self, f: impl FnOnce(&RecoveryOperation) -> T) -> Option<T> {
        let state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation.as_deref().map(f)
    }
    /// Move the genuine completed Main's cause without requiring native cleanup success.
    /// The caller observes Main delivery first; an active native loan keeps this slot absent.
    /// Original context backing and native source custody remain independently retained.
    pub(super) fn take_failure(&self) -> Option<RecoveryFailure> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let operation = state.operation.as_deref_mut()?;
        let original = operation.original.as_ref()?.clone();
        let cause = if let Some(cause) = operation.first.take() {
            RecoveryCause::Store(cause)
        } else if let Some(cause) = operation.memory.take() {
            RecoveryCause::Memory(cause)
        } else if let Some(cause) = operation.retirement.take() {
            RecoveryCause::Store(cause)
        } else {
            return None;
        };
        operation.cause_transferred = true;
        Some(RecoveryFailure { cause, retirement: None, _original: original })
    }
    /// Transfer only the independent original cleanup cell, never a new observation or retry.
    pub(super) fn take_retirement_failure(
        &self,
        original: &MemoryLease,
    ) -> Option<IncidentStoreError> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let operation = state.operation.as_deref_mut()?;
        if !operation.cause_transferred
            || operation.original.as_ref().is_none_or(|held| !held.same_reservation(original))
        {
            return None;
        }
        operation.retirement.take()
    }
    pub(super) fn release(&self) -> Result<(), RecoveryIssue> {
        let operation = {
            let mut state =
                self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if state
                .operation
                .as_ref()
                .is_none_or(|operation| !operation.retired || operation.failed())
            {
                return Err(RecoveryIssue::Busy);
            }
            state.operation.take()
        };
        drop(operation);
        Ok(())
    }
}
pub(super) struct Loan {
    source: Source,
    operation: Option<Box<RecoveryOperation>>,
}
impl Loan {
    pub(super) fn operation(&mut self) -> Result<&mut RecoveryOperation, RecoveryIssue> {
        self.operation.as_deref_mut().ok_or(RecoveryIssue::Busy)
    }
}
impl Drop for Loan {
    fn drop(&mut self) {
        let mut state =
            self.source.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation = self.operation.take();
    }
}
