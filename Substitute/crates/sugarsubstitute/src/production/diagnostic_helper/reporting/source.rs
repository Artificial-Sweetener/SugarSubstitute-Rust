// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore unique report custody on unwind without holding a lock across external providers.
use super::*;
use std::sync::{Arc, Mutex};
struct State {
    input: Option<ReportingInput>,
    operation: Option<Box<ReportingOperation>>,
    started: bool,
    views: u8,
    closed: bool,
}
struct Shared {
    state: Mutex<State>,
    owner: ExecutionId,
    _claim: Option<Arc<MemoryClaim>>,
}
pub(super) struct Source {
    shared: Arc<Shared>,
    outside: MemoryLease,
}
/// Bound replacement overlap, the one sealed result move/box, all control and allocation metadata.
pub(super) const REPORT_SOURCE_PEAK_BYTES: usize = 2 * size_of::<Shared>()
    + HostFailureFact::MAX_HEAP_BYTES
    + size_of::<ReportingOperation>()
    + 6 * size_of::<usize>()
    + size_of::<ReportingService>()
    + ReportingService::passive_retention_bytes()
    + size_of::<MemoryClaim>()
    + 2 * size_of::<SymbolReportObservation>()
    + 4 * size_of::<usize>();
const _: () = assert!(REPORT_SOURCE_PEAK_BYTES < REPORT_SOURCE_BYTES as usize);
impl Source {
    pub(super) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        let claim = context.memory().claim(REPORT_SOURCE_BYTES)?;
        Ok(Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    input: None,
                    operation: Some(Box::new(ReportingOperation::new())),
                    started: false,
                    views: 0,
                    closed: false,
                }),
                owner: context.owner_id(),
                _claim: Some(Arc::new(claim)),
            }),
            outside: context.memory().clone(),
        })
    }
    /// Reserve the existing sole role before allocating one complete Symbols observation.
    /// Claim aliases fence replacement without creating a self-referential Source lifetime.
    pub(super) fn box_symbols(
        &self,
        observation: &mut Option<SymbolReportObservation>,
    ) -> Result<ReportingSymbols, ReportingIssue> {
        let (observation, claim) = {
            let state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let claim = self.shared._claim.as_ref().ok_or(ReportingIssue::Identity)?;
            if state.closed
                || state.started
                || state.input.is_some()
                || state.operation.as_ref().is_none_or(|operation| !operation.pristine())
                || Arc::strong_count(claim) != 1
            {
                return Err(ReportingIssue::Busy);
            }
            let observation = observation.take().ok_or(ReportingIssue::Identity)?;
            (observation, Arc::clone(claim))
        };
        Ok(ReportingSymbols::new(observation, claim))
    }
    pub(super) fn owner(&self) -> ExecutionId {
        self.shared.owner
    }
    pub(super) fn reserve_view(&self) -> Result<ViewSlot, ReportingIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed || state.views >= 2 {
            return Err(ReportingIssue::Busy);
        }
        state.views += 1;
        Ok(ViewSlot { source: self.alias() })
    }
    pub(super) fn alias(&self) -> Self {
        Self { shared: self.shared.clone(), outside: self.outside.clone() }
    }
    pub(super) fn install(&self, input: &mut Option<ReportingInput>) -> Result<(), ReportingIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed
            || state.started
            || state.input.is_some()
            || state
                .operation
                .as_ref()
                .is_none_or(|operation| operation.failed() || operation.retiring_authentication)
        {
            return Err(ReportingIssue::Started);
        }
        state.input = Some(input.take().ok_or(ReportingIssue::Identity)?);
        Ok(())
    }
    pub(super) fn begin(&self) -> Result<(Loan, ReportingInput), ReportingIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed || state.started || state.operation.is_none() {
            return Err(ReportingIssue::Started);
        }
        let input = state.input.take().ok_or(ReportingIssue::Identity)?;
        let operation = state.operation.take().ok_or(ReportingIssue::Busy)?;
        state.started = true;
        Ok((Loan { source: self.alias(), operation: Some(operation) }, input))
    }
    pub(super) fn loan(&self) -> Result<Loan, ReportingIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            return Err(ReportingIssue::Started);
        }
        let operation = state.operation.take().ok_or(ReportingIssue::Busy)?;
        Ok(Loan { source: self.alias(), operation: Some(operation) })
    }
    pub(super) fn inspect<T>(&self, observe: impl FnOnce(&ReportingOperation) -> T) -> Option<T> {
        let state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation.as_deref().map(observe)
    }
    pub(super) fn take_view(&self) -> Option<Box<PreparedViewContent>> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation.as_deref_mut().and_then(ReportingOperation::take_view)
    }
    pub(super) fn take_authentication(&self) -> Option<ReportingAuthentication> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation.as_deref_mut().and_then(ReportingOperation::take_authentication)
    }
    pub(super) fn take_failure(&self, memory: MemoryLease) -> Option<OwnedReportingFailure> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation.as_deref_mut()?.take_failure(memory)
    }
    /// Return the identical passive input and seal this source before releasing its lock.
    /// No native files, completion or source-credit retirement is projected by this operation.
    pub(super) fn restore_passive(&self) -> Result<ReportingInput, ReportingIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed
            || state.started
            || state.operation.as_ref().is_none_or(|operation| !operation.pristine())
        {
            return Err(ReportingIssue::Started);
        }
        let input = state.input.take().ok_or(ReportingIssue::Identity)?;
        state.closed = true;
        Ok(input)
    }
    /// Return passive authentication only after the service verifies real terminal and close custody.
    /// The caller disposes the returned frame after actual same-process retirement, outside locks.
    pub(super) fn take_retiring_authentication(&self) -> Option<ReportingAuthentication> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let operation = state.operation.as_deref_mut()?;
        if operation.retiring_authentication {
            return None;
        }
        let authentication = operation
            .input
            .as_mut()
            .and_then(|input| input.authentication.take())
            .or_else(|| state.input.as_mut().and_then(|input| input.authentication.take()))?;
        state.operation.as_deref_mut()?.retiring_authentication = true;
        Some(authentication)
    }
    pub(super) fn release_adopted(&self) -> Result<(), ReportingIssue> {
        let operation = {
            let mut state =
                self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let operation = state.operation.as_ref().ok_or(ReportingIssue::Busy)?;
            if !state.started
                || !operation.files_retired()
                || operation.failed()
                || operation.retiring_authentication
                || operation.view.is_some()
                || operation.input.as_ref().is_some_and(|input| input.authentication.is_some())
                || (operation.loaded.is_none()
                    && operation.attempt.is_none()
                    && operation.symbols.is_none()
                    && operation.acknowledgement.is_none())
            {
                return Err(ReportingIssue::Busy);
            }
            state.started = false;
            state.operation.take()
        };
        drop(operation);
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation = Some(Box::new(ReportingOperation::new()));
        Ok(())
    }
    /// Observe a uniquely held open generation that has never received this operation's input.
    /// This is ownership inspection only; it grants no file or native retirement proof.
    pub(super) fn uninstalled(&self) -> bool {
        if Arc::strong_count(&self.shared) != 1 {
            return false;
        }
        let state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        !state.closed
            && !state.started
            && state.views == 0
            && state.input.is_none()
            && state.operation.as_ref().is_some_and(|operation| operation.pristine())
            && self.shared._claim.as_ref().is_some_and(|claim| Arc::strong_count(claim) == 1)
    }
    /// Inspect the actual uniquely held closed generation without creating a replacement or authority.
    pub(super) fn passive_replacement_ready(&self) -> bool {
        if Arc::strong_count(&self.shared) != 1 {
            return false;
        }
        let state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.closed
            && !state.started
            && state.views == 0
            && state.input.is_none()
            && state.operation.as_ref().is_some_and(|operation| operation.pristine())
            && self.shared._claim.as_ref().is_some_and(|claim| Arc::strong_count(claim) == 1)
    }
    /// Transfer the existing sole claim into a distinct source; the old closed generation is destroyed.
    /// The original inert operation drops before replacement allocation, outside any provider lock.
    pub(super) fn replace_passive_generation(&mut self) -> Result<(), ReportingIssue> {
        if !self.passive_replacement_ready() {
            return Err(ReportingIssue::Busy);
        }
        let shared = Arc::get_mut(&mut self.shared).ok_or(ReportingIssue::Busy)?;
        let state = shared.state.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
        let claim = shared._claim.take().ok_or(ReportingIssue::Identity)?;
        let owner = shared.owner;
        drop(state.operation.take());
        let fresh = Arc::new(Shared {
            state: Mutex::new(State {
                input: None,
                operation: Some(Box::new(ReportingOperation::new())),
                started: false,
                views: 0,
                closed: false,
            }),
            owner,
            _claim: Some(claim),
        });
        drop(std::mem::replace(&mut self.shared, fresh));
        Ok(())
    }
}
/// Nonclone slot keeps the original controller alive through actual native view disposal.
pub(super) struct ViewSlot {
    source: Source,
}
impl Drop for ViewSlot {
    fn drop(&mut self) {
        let mut state =
            self.source.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.views -= 1;
    }
}
pub(super) struct Loan {
    source: Source,
    operation: Option<Box<ReportingOperation>>,
}
impl Loan {
    pub(super) fn operation(&mut self) -> Result<&mut ReportingOperation, ReportingIssue> {
        self.operation.as_deref_mut().ok_or(ReportingIssue::Busy)
    }
}
impl Drop for Loan {
    fn drop(&mut self) {
        let mut state =
            self.source.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation = self.operation.take();
    }
}
