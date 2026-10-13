// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep each fixed destination charged through its actual owning data lifetime.
use super::*;

#[derive(Clone)]
pub(in crate::diagnostic_storage) struct Binding {
    key: LogicalStoreOperationKey,
    context: JobContext,
    last: Moment,
    deadline: Moment,
}
impl Binding {
    pub(in crate::diagnostic_storage) fn from_operation(binding: &super::super::Binding) -> Self {
        Self {
            key: binding.key,
            context: binding.context.clone(),
            last: binding.last_observed,
            deadline: binding.deadline,
        }
    }
    pub(super) const fn key(&self) -> LogicalStoreOperationKey {
        self.key
    }
    pub(super) fn check(
        &mut self,
        owner: &ledger::Owner,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let observation = self.observe(context);
        self.validate(owner, observation)
    }
    pub(super) fn matches_context(&self, context: &JobContext) -> bool {
        context.owner_id() == self.context.owner_id()
            && context.scope_id() == self.context.scope_id()
    }
    pub(super) fn observe(&self, context: &JobContext) -> Observation {
        Observation {
            execution: context.owner_id(),
            scope: context.scope_id(),
            deadline: context.deadline(),
            original: self.context.now(),
            now: context.now(),
            original_cancelled: self.context.cancellation_requested(),
            cancelled: context.cancellation_requested(),
        }
    }
    pub(super) fn observe_original(&self) -> Observation {
        let mut observation = self.observe(&self.context);
        observation.deadline = Some(self.deadline);
        observation
    }
    pub(super) fn validate(
        &mut self,
        owner: &ledger::Owner,
        observation: Observation,
    ) -> Result<(), DiagnosticStorageError> {
        if observation.execution != self.context.owner_id()
            || observation.scope != self.context.scope_id()
            || !owner.matches(self.key)
        {
            return Err(error(self.key, Code::InvalidIdentity));
        }
        self.validate_observation(observation).map_err(|failure| {
            if failure.closes_admission {
                owner.close();
            }
            error(self.key, failure.code)
        })
    }
    pub(super) fn validate_observation(
        &mut self,
        observation: Observation,
    ) -> Result<(), BindingRejection> {
        if observation.execution != self.context.owner_id()
            || observation.scope != self.context.scope_id()
        {
            return Err(BindingRejection::new(Code::InvalidIdentity, false));
        }
        let until =
            observation.deadline.ok_or_else(|| BindingRejection::new(Code::Admission, false))?;
        let original = observation.original;
        let now = observation.now;
        if until > self.deadline || original < self.last || now < original {
            return Err(BindingRejection::new(Code::Deadline, true));
        }
        self.last = now;
        self.deadline = self.deadline.min(until);
        if now >= self.deadline {
            return Err(BindingRejection::new(Code::Deadline, false));
        }
        if observation.original_cancelled || observation.cancelled {
            return Err(BindingRejection::new(Code::Cancelled, false));
        }
        Ok(())
    }
}
pub(super) struct BindingRejection {
    pub(super) code: Code,
    pub(super) closes_admission: bool,
}
impl BindingRejection {
    const fn new(code: Code, closes_admission: bool) -> Self {
        Self { code, closes_admission }
    }
}
pub(super) struct Observation {
    execution: ExecutionId,
    scope: ScopeId,
    deadline: Option<Moment>,
    original: Moment,
    now: Moment,
    original_cancelled: bool,
    cancelled: bool,
}

/// Retain the closed escaped Summary destination under its original store claim.
pub struct DiagnosticRecoveryRecordLoan {
    pub(super) _allocation: ledger::Residual,
}
/// Retain the closed escaped seed destination under its original store claim.
pub struct DiagnosticRecoverySeedLoan {
    pub(super) _allocation: ledger::Residual,
}
/// Retain the complete seed/context of an admitted publication preflight.
pub struct DiagnosticRetainedPreflightLoan {
    pub(super) _allocation: ledger::Residual,
}
/// Retain pending seed/context and its literal attachment tuple capacity.
pub struct DiagnosticPendingPublicationLoan {
    pub(super) _allocation: ledger::Residual,
}

/// Retain the complete closed callback decision destination before its data is built.
/// The private victim vector has its full capacity admitted before allocation.
pub struct DiagnosticPayloadDecisionAllocationLoan {
    pub(in crate::diagnostic_storage) victims: Vec<DiagnosticPayloadVictimExpectation>,
    pub(super) key: LogicalStoreOperationKey,
    pub(super) _allocation: ledger::Residual,
}
impl DiagnosticPayloadDecisionAllocationLoan {
    pub(in crate::diagnostic_storage) fn matches(
        &self,
        operation: &DiagnosticStorageOperation,
    ) -> bool {
        self.key == operation.key
            && self._allocation.owner.same(&operation.shared._allocation)
            && self._allocation.owner.matches(operation.key)
    }
}

pub(in crate::diagnostic_storage) struct RetainedIntent<T> {
    data: Box<T>,
    _allocation: ledger::Residual,
}
impl<T> std::ops::Deref for RetainedIntent<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.data
    }
}
impl<T> std::ops::DerefMut for RetainedIntent<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}
impl ledger::Residual {
    pub(in crate::diagnostic_storage) fn payload_intent(
        self,
        data: payload_discard::intent::PayloadIntent,
    ) -> RetainedIntent<payload_discard::intent::PayloadIntent> {
        RetainedIntent { data: Box::new(data), _allocation: self }
    }
    pub(in crate::diagnostic_storage) fn migration_intent(
        self,
        data: summary_migration::MigrationIntent,
    ) -> RetainedIntent<summary_migration::MigrationIntent> {
        RetainedIntent { data: Box::new(data), _allocation: self }
    }
}

/// Correlate one repository and its tokens without granting storage authority.
/// The actual Arc allocation drops before its final capacity loan.
pub struct DiagnosticPublicationIdentity {
    pub(super) identity: Arc<()>,
    pub(super) binding: Mutex<Binding>,
    pub(super) allocation: IdentityLoan,
}
pub(super) struct IdentityLoan {
    pub(super) control: Option<Arc<()>>,
    pub(super) token: ledger::Residual,
}
impl Drop for IdentityLoan {
    fn drop(&mut self) {
        if let Some(control) = self.control.take() {
            self.token.owner.retire_correlation(control);
        }
    }
}
impl DiagnosticPublicationIdentity {
    /// Compare the existing correlation allocation; equality grants no authority.
    pub fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
    /// Admit a token destination before aliasing the original correlation.
    /// # Errors
    /// Rejects foreign/expired/currently closed operations and exhausted residual.
    pub fn retain_for_token(
        &self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<Self, DiagnosticStorageError> {
        let snapshot =
            self.binding.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        let observation = snapshot.observe(context);
        let mut binding = self.binding.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if operation.key != binding.key()
            || !self.allocation.token.owner.same(&operation.shared._allocation)
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        binding.validate(&self.allocation.token.owner, observation)?;
        let token = self
            .allocation
            .token
            .owner
            .residual(roles::ResidualRole::PublicationToken, operation.key)?;
        let next = binding.clone();
        Ok(Self {
            identity: self.identity.clone(),
            binding: Mutex::new(next),
            allocation: IdentityLoan { control: Some(self.identity.clone()), token },
        })
    }
}
