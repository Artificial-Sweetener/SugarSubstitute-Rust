// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore exact native custody on unwinding and keep metadata charged outside Arc allocations.
use super::*;
use operation::Publication;
struct State {
    input: Option<Input>,
    operation: Option<Box<Publication>>,
    used: bool,
    completed: Option<Completed>,
}
struct Completed {
    grant: RegisteredRecoveryGrant,
    published: Published,
}
struct Shared {
    state: Mutex<State>,
    execution: ExecutionId,
}
struct Charge {
    claim: MemoryClaim,
}
/// Each alias drops data/control before its outside original projection claim.
pub(super) struct Source {
    shared: Option<Arc<Shared>>,
    charge: Option<Arc<Charge>>,
}
/// Exact static peak: actual service, retained operation, both Arc controls and one loan.
/// Existing native repository buffers retain their separate actual8MiB allocation claim.
pub(super) const fn storage_bytes() -> u64 {
    (size_of::<Shared>()
        + size_of::<Publication>()
        + size_of::<Charge>()
        + size_of::<super::service::OriginPublication>()
        + size_of::<Loan>()
        + 4 * size_of::<usize>()
        + 2 * (size_of::<failure::FailureData>() + 2 * size_of::<usize>())) as u64
}
const _: () = assert!(storage_bytes() <= 16 * 1024);
impl Source {
    pub(super) fn new(input: OriginPublicationInput, execution: ExecutionId) -> Self {
        let OriginPublicationInput { claim, grant, capture, request, operation, at } = input;
        let charge = Arc::new(Charge { claim });
        let shared = Arc::new(Shared {
            execution,
            state: Mutex::new(State {
                input: Some(Input { grant, capture, request, operation, at }),
                operation: Some(Box::new(Publication::new())),
                used: false,
                completed: None,
            }),
        });
        Self { shared: Some(shared), charge: Some(charge) }
    }
    fn shared(&self) -> Result<&Arc<Shared>, OriginIssue> {
        self.shared.as_ref().ok_or(OriginIssue::Identity)
    }
    pub(super) fn owner(&self) -> Result<ExecutionId, OriginIssue> {
        Ok(self.shared()?.execution)
    }
    pub(super) fn alias(&self) -> Result<Self, OriginIssue> {
        Ok(Self {
            shared: Some(self.shared()?.clone()),
            charge: Some(self.charge.as_ref().ok_or(OriginIssue::Identity)?.clone()),
        })
    }
    /// Keep providers outside this mutex; the loan alone temporarily owns actual inputs and outputs.
    pub(super) fn loan(&self, begin: bool) -> Result<Loan, OriginIssue> {
        let source = self.alias()?;
        let mut state = lock(&self.shared()?.state);
        if begin && state.used {
            return Err(OriginIssue::Started);
        }
        let operation = state.operation.take().ok_or(OriginIssue::Busy)?;
        let input = state.input.take();
        if begin {
            state.used = true;
        }
        Ok(Loan { source, input, operation: Some(operation) })
    }
    pub(super) fn until(&self) -> Result<Moment, OriginIssue> {
        let state = lock(&self.shared()?.state);
        Ok(state.input.as_ref().ok_or(OriginIssue::Busy)?.grant.original_budget().until())
    }
    pub(super) fn matches(&self, grant: &RegisteredRecoveryGrant) -> Result<bool, OriginIssue> {
        let state = lock(&self.shared()?.state);
        Ok(state.input.as_ref().is_some_and(|input| input.grant.same_grant(grant))
            || state.completed.as_ref().is_some_and(|done| done.grant.same_grant(grant)))
    }
    pub(super) fn failure(&self, cleanup: bool) -> Option<OriginFailure> {
        let shared = self.shared.as_ref()?;
        let cause = {
            let state = lock(&shared.state);
            state.operation.as_ref()?.failure(cleanup)?
        };
        Some(OriginFailure { cause, _source: self.alias().ok()? })
    }
    pub(super) fn retired(&self) -> bool {
        self.shared.as_ref().is_some_and(|shared| {
            let state = lock(&shared.state);
            state.completed.is_some()
                || state.operation.as_ref().is_some_and(|operation| operation.retired())
        })
    }
    /// Copy only the private completion from the actual checked read after real successful cleanup.
    pub(super) fn compact(&mut self) -> Result<(), OriginIssue> {
        let (operation, input) = {
            let mut state = lock(&self.shared()?.state);
            if state.completed.is_some() {
                return Ok(());
            }
            let published = state
                .operation
                .as_ref()
                .and_then(|operation| operation.completed())
                .ok_or(OriginIssue::Busy)?;
            let input = state.input.take().ok_or(OriginIssue::Identity)?;
            if published.key != input.grant.key()
                || published.operation != input.operation
                || published.origin != RecoveryCaptureIdentity::from_grant(input.grant.key())
            {
                state.input = Some(input);
                return Err(OriginIssue::Identity);
            }
            let Input { grant, capture, request, operation: key, .. } = input;
            state.completed = Some(Completed { grant, published });
            (state.operation.take(), (capture, request, key))
        };
        drop(operation);
        drop(input);
        Ok(())
    }
    pub(super) fn ready(&self, grant: &RegisteredRecoveryGrant) -> Result<bool, OriginIssue> {
        let state = lock(&self.shared()?.state);
        Ok(state.completed.as_ref().is_some_and(|done| {
            done.grant.same_grant(grant)
                && done.published.key == grant.key()
                && done.published.root_identity != [0; 24]
                && done.published.journal.bytes() > 0
        }))
    }
}
/// This allocation remains in its original source across panic, cancellation and discarded outcomes.
pub(super) struct Loan {
    source: Source,
    input: Option<Input>,
    operation: Option<Box<Publication>>,
}
impl Loan {
    pub(super) fn execute(&mut self, context: &JobContext) -> Result<(), OriginIssue> {
        let input = self.input.as_ref().ok_or(OriginIssue::Identity)?;
        self.operation.as_mut().ok_or(OriginIssue::Busy)?.execute(input, context)
    }
    pub(super) fn retire(&mut self, context: &JobContext) -> Result<bool, OriginIssue> {
        self.operation.as_mut().ok_or(OriginIssue::Busy)?.retire(context)
    }
}
impl Drop for Loan {
    fn drop(&mut self) {
        let Some(shared) = self.source.shared.as_ref() else {
            return;
        };
        let mut state = lock(&shared.state);
        if state.operation.is_none() && state.input.is_none() {
            state.operation = self.operation.take();
            state.input = self.input.take();
        } else {
            // Conflicting custody cannot authorize freeing partial native outputs or original input.
            if let Some(operation) = self.operation.take() {
                std::mem::forget(operation);
            }
            if let Some(input) = self.input.take() {
                std::mem::forget(input);
            }
            if let Some(charge) = self.source.charge.take() {
                std::mem::forget(charge);
            }
        }
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.take().and_then(Arc::into_inner) {
            let releasable = lock(&shared.state)
                .operation
                .as_ref()
                .is_none_or(|operation| operation.releasable());
            if !releasable {
                std::mem::forget(shared);
                if let Some(charge) = self.charge.take() {
                    std::mem::forget(charge);
                }
                tracing::error!(
                    operation = "recovery_origin_uncertain_drop",
                    "Original input, partial publication and metadata claim retained before diagnostics"
                );
                return;
            }
            drop(shared);
        }
        if let Some(charge) = self.charge.take().and_then(Arc::into_inner) {
            drop(charge.claim);
        }
    }
}
#[cfg(test)]
#[path = "source_tests.rs"]
pub(super) mod tests;
