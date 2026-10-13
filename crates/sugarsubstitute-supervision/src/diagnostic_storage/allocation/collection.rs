// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer each selected parsed record into an independent residual destination.
use super::*;

/// Own the sole fixed collection phase, including both literal128 vectors.
/// Release this loan only after all remaining records and vector allocations drop.
pub struct DiagnosticRecoveryCollectionLoan {
    pub(super) binding: loans::Binding,
    pub(super) issued: [u64; 2],
    pub(super) phase: ledger::PhaseLoan,
}
/// Retain the original collection phase through a failed allocation's cause.
/// This compact owner grants no operation, context, record transfer or native access.
pub struct DiagnosticRecoveryCollectionErrorLoan {
    _phase: ledger::PhaseLoan,
}
impl DiagnosticRecoveryCollectionLoan {
    /// Move the original phase into a compact failure owner without allocating.
    /// Destroy every collection-owned vector and record before this transfer.
    /// The original context drops before return; the phase stays occupied until
    /// the returned owner drops after the genuine cause and any retained data.
    pub fn into_error(self) -> DiagnosticRecoveryCollectionErrorLoan {
        let Self { binding, issued: _, phase } = self;
        drop(binding);
        DiagnosticRecoveryCollectionErrorLoan { _phase: phase }
    }

    /// Admit an escaped destination before moving one original parsed record.
    /// # Errors
    /// Rejects reused/out-of-bound indices, foreign/expired context or saturation.
    pub fn transfer_record(
        &mut self,
        index: u8,
        context: &JobContext,
    ) -> Result<DiagnosticRecoveryRecordLoan, DiagnosticStorageError> {
        let key = self.binding.key();
        if usize::from(index) >= sugarsubstitute_contracts::incident::MAX_SUMMARIES {
            return Err(error(key, Code::Capacity));
        }
        self.binding.check(&self.phase.owner, context)?;
        let cell = usize::from(index / 64);
        let bit = 1u64 << (index % 64);
        if self.issued[cell] & bit != 0 {
            return Err(error(key, Code::InvalidIdentity));
        }
        let allocation = self.phase.owner.residual(roles::ResidualRole::Record, key)?;
        self.issued[cell] |= bit;
        Ok(DiagnosticRecoveryRecordLoan { _allocation: allocation })
    }
}
