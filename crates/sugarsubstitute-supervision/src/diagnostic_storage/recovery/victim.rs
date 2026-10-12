// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe an actual recovered victim state without manufacturing disposition authority.
use super::*;
/// Observe checked retained recovery facts; matching data grants no native rights.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticRecoveredVictimState {
    /// The exact DELETE-capable victim remains retained and requires checked settlement.
    Retained,
    /// Protected absence or checked deletion/closure was actually established under the intent.
    Settled,
}
impl DiagnosticStorage {
    /// Observe a recovered intent using its actual opaque permit and current original context.
    /// Already issued deletion leases remain Retained until actual settlement.
    /// # Errors
    /// Rejects foreign, ordinary, retired, cancelled, expired or uncertain native custody.
    pub fn recovered_victim_state(
        &mut self,
        permit: &mut DurableDeletionPermit,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticRecoveredVictimState, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        deletion::permit_check(&state, &self.shared, permit, operation.key)?;
        let retained = state.deletion.as_ref().ok_or_else(|| error(operation.key, Code::Closed))?;
        let (case, _, _) =
            retained.recovery.ok_or_else(|| error(operation.key, Code::InvalidIdentity))?;
        let (journal, journal_facts) =
            retained.journal.ok_or_else(|| error(operation.key, Code::Closed))?;
        let (settled, victim, victim_facts, expected) =
            (retained.victim_settled, retained.victim, retained.victim_facts, retained.plan.facts);
        let actual = state
            .port
            .observe_file(journal, context)
            .map_err(|cause| native(operation.key, Code::Read, cause))?;
        if actual != journal_facts {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        check(&mut state, operation.key, context, false)?;
        if settled {
            if victim.is_some() {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            Ok(DiagnosticRecoveredVictimState::Settled)
        } else {
            if case != DiagnosticRecoveryLedgerCase::Original {
                return Err(error(operation.key, Code::InvalidIdentity));
            }
            let id = victim.ok_or_else(|| error(operation.key, Code::Closed))?;
            let actual = state
                .port
                .observe_file(id, context)
                .map_err(|cause| native(operation.key, Code::Read, cause))?;
            if victim_facts != Some(actual) || !expected.matches(actual) {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            check(&mut state, operation.key, context, false)?;
            Ok(DiagnosticRecoveredVictimState::Retained)
        }
    }
}
