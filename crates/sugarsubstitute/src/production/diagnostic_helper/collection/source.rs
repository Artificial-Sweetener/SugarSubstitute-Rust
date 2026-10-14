// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore the identical funded policy and partial native transaction outside provider locks.
use super::{CollectionInput, CollectionOperation};
use crate::production::incident_host::FundedState;
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::*;
pub(in crate::production) const COLLECTION_SOURCE_BYTES: u64 = 16384;
struct State {
    input: Option<CollectionInput>,
    policy: Option<FundedState>,
    operation: Option<Box<CollectionOperation>>,
    started: bool,
}
impl State {
    /// Move the identical controller without constructing replacement state or admitting work.
    /// The public source path additionally requires its genuine occupied original input.
    fn restore_unstarted_policy(
        &mut self,
        output: &mut Option<FundedState>,
    ) -> Result<Option<CollectionInput>, super::service::CollectionIssue> {
        if output.is_some()
            || self.started
            || self.policy.is_none()
            || self
                .policy
                .as_ref()
                .is_some_and(|policy| policy.state.effects.iter().any(Option::is_some))
            || self.operation.as_ref().is_none_or(|operation| operation.scope().is_some())
        {
            return Err(super::service::CollectionIssue::Busy);
        }
        *output = self.policy.take();
        Ok(self.input.take())
    }
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
        + size_of::<CollectionOperation>()
        + size_of::<super::service::CollectionService>()
        + 2 * size_of::<usize>()
        < COLLECTION_SOURCE_BYTES as usize
);
impl Source {
    /// Restore only a genuine input that never entered its unique execution loan.
    /// Drop the original passive namespace/Capture guards outside the source lock.
    /// This returns inert correlation, never a file-retirement or completion proof.
    pub(super) fn abandon_unsubmitted(
        &self,
        output: &mut Option<FundedState>,
    ) -> Result<
        sugarsubstitute_application::incident::IncidentOperationScope,
        super::service::CollectionIssue,
    > {
        let input = {
            let mut state =
                self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.input.is_none() {
                return Err(super::service::CollectionIssue::Busy);
            }
            state.restore_unstarted_policy(output)?.ok_or(super::service::CollectionIssue::Busy)?
        };
        let scope = input.scope;
        drop(input);
        Ok(scope)
    }
    pub(super) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        let claim = context.memory().claim(COLLECTION_SOURCE_BYTES)?;
        Ok(Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    input: None,
                    policy: None,
                    operation: Some(Box::new(CollectionOperation::new())),
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
    pub(super) fn install(
        &self,
        input: &mut Option<CollectionInput>,
        policy: &mut Option<FundedState>,
    ) -> Result<(), super::service::CollectionIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.started
            || state.input.is_some()
            || state.policy.is_some()
            || input.is_none()
            || policy.is_none()
        {
            return Err(super::service::CollectionIssue::Busy);
        }
        if state.operation.is_none() {
            state.operation = Some(Box::new(CollectionOperation::new()));
        }
        state.input = input.take();
        state.policy = policy.take();
        Ok(())
    }
    pub(super) fn release_retired(&self) -> Result<(), super::service::CollectionIssue> {
        let operation = {
            let mut state =
                self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.policy.is_some()
                || state.operation.as_ref().is_none_or(|operation| !operation.releasable())
            {
                return Err(super::service::CollectionIssue::Busy);
            }
            let operation = state.operation.take();
            state.started = false;
            operation
        };
        drop(operation);
        Ok(())
    }
    pub(super) fn begin(&self) -> Result<(Loan, CollectionInput), super::service::CollectionIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.started
            || state.operation.is_none()
            || state.policy.is_none()
            || state.input.is_none()
        {
            return Err(super::service::CollectionIssue::Busy);
        }
        let input = state.input.take().ok_or(super::service::CollectionIssue::Busy)?;
        let operation = state.operation.take();
        let policy = state.policy.take();
        state.started = true;
        Ok((Loan { source: self.alias(), operation, policy }, input))
    }
    pub(super) fn loan(&self) -> Result<Loan, super::service::CollectionIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.operation.is_none() {
            return Err(super::service::CollectionIssue::Busy);
        }
        Ok(Loan { source: self.alias(), operation: state.operation.take(), policy: None })
    }
    /// Restore after the service validates and retains the actual matching terminal completion.
    /// A worker that never entered begin returns passive correlation without file-retirement proof.
    pub(super) fn restore(
        &self,
        output: &mut Option<FundedState>,
    ) -> Result<
        Option<sugarsubstitute_application::incident::IncidentOperationScope>,
        super::service::CollectionIssue,
    > {
        let unused = {
            let mut state =
                self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.started {
                if output.is_some() || state.operation.is_none() || state.policy.is_none() {
                    return Err(super::service::CollectionIssue::Busy);
                }
                *output = state.policy.take();
                return Ok(None);
            }
            if state.input.is_none() {
                return Err(super::service::CollectionIssue::Busy);
            }
            state.restore_unstarted_policy(output)?.ok_or(super::service::CollectionIssue::Busy)?
        };
        let scope = unused.scope;
        drop(unused);
        Ok(Some(scope))
    }
    pub(super) fn inspect<T>(&self, read: impl FnOnce(&CollectionOperation) -> T) -> Option<T> {
        let state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation.as_deref().map(read)
    }
}
pub(super) struct Loan {
    source: Source,
    operation: Option<Box<CollectionOperation>>,
    policy: Option<FundedState>,
}
impl Loan {
    pub(super) fn execute(
        &mut self,
        input: CollectionInput,
        context: &JobContext,
    ) -> Result<(), super::service::CollectionIssue> {
        let operation = self.operation.as_mut().ok_or(super::service::CollectionIssue::Busy)?;
        let policy = self.policy.as_mut().ok_or(super::service::CollectionIssue::Busy)?;
        operation.execute(input, &mut policy.state.coordinator, &mut policy.state.effects, context);
        Ok(())
    }
    pub(super) fn retire(
        &mut self,
        client: &sugarsubstitute_execution::ExecutionClient,
        context: &JobContext,
    ) -> Result<bool, super::service::CollectionIssue> {
        Ok(self
            .operation
            .as_mut()
            .ok_or(super::service::CollectionIssue::Busy)?
            .retire(client, context))
    }
}
impl Drop for Loan {
    fn drop(&mut self) {
        let mut state =
            self.source.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.operation = self.operation.take();
        if self.policy.is_some() {
            state.policy = self.policy.take();
        }
    }
}

#[cfg(test)]
mod restoration_tests {
    use super::*;
    /// Test only owner/control restoration; no fake FinishedCaptureSource is constructed.
    #[test]
    fn collection_unsubmitted_policy_restoration_preserves_actual_funded_identity_and_credit()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::production::incident_host::tests::admitted_value(
            crate::production::incident_host::CONTROLLER_BYTES,
            |context| {
                let funded =
                    crate::production::incident_host::tests::collection_policy_fixture(&context)?;
                let original = std::ptr::from_ref(&*funded.state);
                let source = Source::new(&context)?;
                let mut output = None;
                {
                    let mut state = source
                        .shared
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    state.policy = Some(funded);
                }
                assert!(source.abandon_unsubmitted(&mut output).is_err());
                assert!(source.restore(&mut output).is_err());
                assert!(output.is_none());
                {
                    let mut state = source
                        .shared
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    assert_eq!(
                        state.policy.as_ref().map(|value| std::ptr::from_ref(&*value.state)),
                        Some(original)
                    );
                    state.started = true;
                    assert!(state.restore_unstarted_policy(&mut output).is_err());
                    assert!(output.is_none());
                    state.started = false;
                    assert!(state.restore_unstarted_policy(&mut output)?.is_none());
                    assert!(state.policy.is_none());
                    assert_eq!(
                        output.as_ref().map(|value| std::ptr::from_ref(&*value.state)),
                        Some(original)
                    );
                    assert!(state.restore_unstarted_policy(&mut output).is_err());
                }
                Ok((source, output))
            },
        )
    }
}
