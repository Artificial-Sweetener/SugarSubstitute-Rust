// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preadmit the fixed genuine reserve-error destination before allocating data.
use super::*;
use crate::claim_error::{BoundedNativeCause, NativeCauseDestination};

pub(in crate::diagnostic_storage) struct ReservationDestination {
    key: Option<LogicalStoreOperationKey>,
    destination: NativeCauseDestination,
}
impl ReservationDestination {
    /// Move the genuine failed reservation into its already-held concrete cause control.
    pub(in crate::diagnostic_storage) fn failed(
        self,
        cause: std::collections::TryReserveError,
    ) -> DiagnosticStorageError {
        DiagnosticStorageError::native(
            self.key,
            Code::Capacity,
            ClaimError::native(
                "diagnostic_reservation",
                BoundedNativeCause::Reserve(cause),
                self.destination,
            ),
        )
    }
}
/// Hold one fixed cause destination under the same original storage ledger.
/// This grants neither data capacity nor native permission.
pub(in crate::diagnostic_storage) fn reservation(
    owner: &DiagnosticAllocationOwner,
    key: Option<LogicalStoreOperationKey>,
) -> Result<ReservationDestination, DiagnosticStorageError> {
    let grant = owner.native_grant();
    let allocation = grant.native_cause().map_err(|cause| {
        DiagnosticStorageError::native(
            key,
            Code::Admission,
            ClaimError::diagnostic_allocation(cause),
        )
    })?;
    Ok(ReservationDestination {
        key,
        destination: NativeCauseDestination::diagnostic(allocation, grant),
    })
}
pub(in crate::diagnostic_storage) fn reservation_for_loan(
    loan: &DiagnosticAllocationLoan,
    key: LogicalStoreOperationKey,
) -> Result<ReservationDestination, DiagnosticStorageError> {
    reservation(&loan.owner, Some(key))
}
