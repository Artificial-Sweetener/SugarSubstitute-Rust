// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit one namespace transaction through its main work and checked native retirement.
use sugarsubstitute_contracts::{
    incident::{IncidentOperationId, LogicalStoreOperationKey},
    lifecycle::CoordinatorKey,
};

/// Identify the existing finite use case retaining the namespace's native capsule or writer fence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::entry_activation) enum DiagnosticTransactionRole {
    Recovery,
    Seed,
    Collection,
    Reporting,
    Symbols,
    Export,
    RecoveryOrigin,
}

/// Keep the complete current owner, operation and role in the sole admission owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TransactionKey {
    role: DiagnosticTransactionRole,
    operation: LogicalStoreOperationKey,
}

/// Retain application admission only; this token grants no native handle, path or effect authority.
/// Its existing service owns every input, execution admission, deadline and cleanup proof.
pub(in crate::production::entry_activation) struct DiagnosticTransaction {
    id: IncidentOperationId,
    role: DiagnosticTransactionRole,
}

const _: () = assert!(size_of::<DiagnosticTransaction>() <= 16);
const _: () = assert!(size_of::<Option<DiagnosticTransaction>>() <= 16);

/// Own namespace transaction admission independently of worker, Reporter and native-pipe pumping.
/// A queued use case retains its original input locally; it creates no second scheduler or queue.
pub(in crate::production::entry_activation) struct DiagnosticTransactions {
    owner: Option<CoordinatorKey>,
    active: Option<TransactionKey>,
}

/// Reject a changed caller or token without releasing the original active transaction.
#[derive(Debug)]
pub(in crate::production::entry_activation) struct DiagnosticTransactionError;
impl std::fmt::Display for DiagnosticTransactionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Diagnostic transaction admission identity changed")
    }
}
impl std::error::Error for DiagnosticTransactionError {}

impl DiagnosticTransactions {
    /// Construct inert admission metadata before consuming the genuine namespace.
    pub(super) const fn new() -> Self {
        Self { owner: None, active: None }
    }

    /// Bind once to the current authenticated coordinator after its genuine namespace is consumed.
    pub(super) fn bind(&mut self, owner: CoordinatorKey) -> Result<(), DiagnosticTransactionError> {
        if self.owner.is_some_and(|original| original != owner) {
            return Err(DiagnosticTransactionError);
        }
        self.owner = Some(owner);
        Ok(())
    }

    /// Inspect application capacity without starting work or changing an original deadline.
    pub(in crate::production::entry_activation) const fn available(&self) -> bool {
        self.active.is_none()
    }

    /// Admit or revalidate this same owned transaction before its first execution submission.
    /// False leaves the identical caller input pending. Foreign/stale tokens reject without mutation.
    pub(in crate::production::entry_activation) fn admit(
        &mut self,
        role: DiagnosticTransactionRole,
        operation: LogicalStoreOperationKey,
        token: &mut Option<DiagnosticTransaction>,
    ) -> Result<bool, DiagnosticTransactionError> {
        if self.owner != Some(operation.owner()) {
            return Err(DiagnosticTransactionError);
        }
        let key = TransactionKey { role, operation };
        if let Some(original) = token {
            if original.id != operation.id() || original.role != role || self.active != Some(key) {
                return Err(DiagnosticTransactionError);
            }
            return Ok(true);
        }
        if self.active.is_some() {
            return Ok(false);
        }
        self.active = Some(key);
        *token = Some(DiagnosticTransaction { id: operation.id(), role });
        Ok(true)
    }

    /// Return the same application token after the service's checked native retirement or
    /// genuine unsubmitted restoration. Revalidate the caller's original full operation key;
    /// outcome delivery or worker return alone is insufficient.
    pub(in crate::production::entry_activation) fn retired(
        &mut self,
        operation: LogicalStoreOperationKey,
        token: &mut Option<DiagnosticTransaction>,
    ) -> Result<(), DiagnosticTransactionError> {
        let Some(original) = token else { return Ok(()) };
        if self.owner != Some(operation.owner())
            || original.id != operation.id()
            || self.active != Some(TransactionKey { role: original.role, operation })
        {
            return Err(DiagnosticTransactionError);
        }
        self.active = None;
        *token = None;
        Ok(())
    }

    /// Observe cached application admission only; this does not establish native settlement.
    pub(in crate::production::entry_activation) const fn settled(&self) -> bool {
        self.active.is_none()
    }
}

#[cfg(test)]
#[path = "transactions/tests.rs"]
mod tests;
