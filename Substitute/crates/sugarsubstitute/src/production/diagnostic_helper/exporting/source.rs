// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore the unique operation on unwind without holding metadata locks across native effects.
use super::{operation::Operation, *};
use std::sync::{Arc, Mutex};
struct State {
    operation: Option<Box<Operation>>,
    started: bool,
}
struct Shared {
    state: Mutex<State>,
    owner: ExecutionId,
    _claim: Option<MemoryClaim>,
}
pub(super) struct Source {
    shared: Arc<Shared>,
    _outside: MemoryLease,
}
const _: () = assert!(
    size_of::<Shared>()
        + size_of::<Operation>()
        + size_of::<ExportService>()
        + 2 * size_of::<usize>()
        < EXPORT_SOURCE_BYTES as usize
);
impl Source {
    pub(super) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        let claim = context.memory().claim(EXPORT_SOURCE_BYTES)?;
        Ok(Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    operation: Some(Box::new(Operation::new())),
                    started: false,
                }),
                owner: context.owner_id(),
                _claim: Some(claim),
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
    pub(super) fn install(&self, input: &mut Option<ExportInput>) -> Result<(), ExportIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.started
            || state.operation.as_ref().is_none_or(|operation| operation.input.is_some())
        {
            return Err(ExportIssue::Started);
        }
        let operation = state.operation.as_mut().ok_or(ExportIssue::Busy)?;
        operation.input = Some(input.take().ok_or(ExportIssue::Identity)?);
        Ok(())
    }
    pub(super) fn loan(&self, begin: bool) -> Result<Loan, ExportIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if begin && state.started {
            return Err(ExportIssue::Started);
        }
        let operation = state.operation.take().ok_or(ExportIssue::Busy)?;
        state.started |= begin;
        Ok(Loan { source: self.alias(), operation: Some(operation) })
    }
    pub(super) fn release_adopted(&self) -> Result<(), ExportIssue> {
        if Arc::strong_count(&self.shared) != 1 {
            return Err(ExportIssue::Busy);
        }
        let old = {
            let mut state =
                self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let operation = state.operation.as_ref().ok_or(ExportIssue::Busy)?;
            if operation.input.is_some() || !operation.source_closed || !operation.receiving_closed
            {
                return Err(ExportIssue::Busy);
            }
            state.started = false;
            state.operation.take()
        };
        drop(old);
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation = Some(Box::new(Operation::new()));
        Ok(())
    }
    /// Reset metadata only after never-started input and cancelled factories have returned.
    pub(super) fn release_unsubmitted(&self) -> Result<(), ExportIssue> {
        if Arc::strong_count(&self.shared) != 1 {
            return Err(ExportIssue::Busy);
        }
        let old = {
            let mut state =
                self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.started || state.operation.as_ref().is_none_or(|op| op.input.is_some()) {
                return Err(ExportIssue::Busy);
            }
            state.operation.take()
        };
        drop(old);
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation = Some(Box::new(Operation::new()));
        Ok(())
    }
    /// Transfer the unchanged input only before any actual Main factory has started.
    pub(super) fn take_unsubmitted_input(&self) -> Result<Option<ExportInput>, ExportIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.started {
            return Err(ExportIssue::Started);
        }
        let operation = state.operation.as_mut().ok_or(ExportIssue::Busy)?;
        if operation.native.is_some()
            || operation.construction_failure.is_some()
            || operation.dump_reader.is_some()
            || operation.export.is_some()
            || operation.rejected_dump.is_some()
        {
            return Err(ExportIssue::Identity);
        }
        Ok(operation.input.take())
    }
    pub(super) fn inspect<T>(&self, observe: impl FnOnce(&Operation) -> T) -> Option<T> {
        let state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation.as_deref().map(observe)
    }
}
pub(super) struct Loan {
    source: Source,
    operation: Option<Box<Operation>>,
}
impl Loan {
    pub(super) fn operation(&mut self) -> Result<&mut Operation, ExportIssue> {
        self.operation.as_deref_mut().ok_or(ExportIssue::Busy)
    }
}
impl Drop for Loan {
    fn drop(&mut self) {
        let mut state =
            self.source.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation = self.operation.take();
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        let state = self.state.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
        let retain = state.operation.as_ref().is_some_and(|operation| {
            !(operation.source_closed && operation.receiving_closed)
                && (operation.native.is_some()
                    || operation.construction_failure.is_some()
                    || operation.dump_reader.is_some()
                    || operation.rejected_dump.is_some()
                    || operation
                        .export
                        .as_ref()
                        .is_some_and(|export| export.custody().native_outputs != 0))
        });
        if retain {
            if let Some(operation) = state.operation.take() {
                std::mem::forget(operation);
            }
            if let Some(claim) = self._claim.take() {
                std::mem::forget(claim);
            }
            tracing::error!(
                operation = "report_export_retained",
                "Original report source and native custody retained"
            );
        }
    }
}
