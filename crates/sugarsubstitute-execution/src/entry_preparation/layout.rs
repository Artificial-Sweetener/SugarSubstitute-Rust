// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound compiled owner/native metadata before allocating beneath the original claim.

use super::{custody::PreparationPayload, slot::PreparationSlot, state::NativeCapture};
use std::sync::atomic::AtomicUsize;
use sugarsubstitute_contracts::coordinator_entry::{
    MAX_ENTRY_ENVIRONMENT_UNITS, MAX_ENTRY_REQUEST_ALLOCATION_BYTES,
};

fn arc_bytes<T>() -> Option<u64> {
    // The pinned standard Arc has two atomic reference counts followed by T.
    // Alignment is included explicitly; allocator overhead/OS extent is not.
    let header = size_of::<AtomicUsize>().checked_mul(2)?;
    let alignment = align_of::<T>().max(align_of::<AtomicUsize>());
    let padded = header.checked_add(alignment.checked_sub(1)?)? / alignment * alignment;
    u64::try_from(padded.checked_add(size_of::<T>())?).ok()
}
pub(super) fn retained_native_bound() -> Option<u64> {
    let units = u64::try_from(MAX_ENTRY_ENVIRONMENT_UNITS.checked_mul(size_of::<u16>())?).ok()?;
    let state = u64::try_from(size_of::<NativeCapture>()).ok()?;
    // Account for the charged closed owner and passive-slot Arc allocations,
    // original native state, full copied capacity and Domain request capacity.
    let errors = arc_bytes::<super::error::FailurePayload>()?.checked_mul(9)?;
    let bounded_panic_bytes = 9 * 1024;
    #[cfg(any(windows, test))]
    let operation_errors = arc_bytes::<super::owner::PreparedOperationSlot>()?
        .checked_add(arc_bytes::<
            crate::coordinator_entry::prepared_failure::PreparedFailurePayload,
        >()?)?
        // The owned transport creates at most one Retirement layer per cause:
        // subsequent native cleanup updates that layer. Include original+eight
        // retained transport boxes plus one in-flight box, and each corresponding
        // converted parent view; no public raw retained wrapper is projected.
        .checked_add(
            u64::try_from(
                size_of::<crate::EntryTransportError>()
                    .checked_add(size_of::<crate::EntryPreparedCause>())?,
            )
            .ok()?
            .checked_mul(10)?,
        )?;
    #[cfg(not(any(windows, test)))]
    let operation_errors = 0;
    arc_bytes::<PreparationPayload>()?
        .checked_add(arc_bytes::<PreparationSlot>()?)?
        .checked_add(arc_bytes::<sugarsubstitute_contracts::execution::JobContext>()?)?
        .checked_add(
            u64::try_from(size_of::<
                sugarsubstitute_contracts::coordinator_entry::EntryLaunchRequest,
            >())
            .ok()?,
        )?
        .checked_add(state)?
        .checked_add(errors)?
        .checked_add(operation_errors)?
        .checked_add(bounded_panic_bytes)?
        .checked_add(units)?
        .checked_add(MAX_ENTRY_REQUEST_ALLOCATION_BYTES)
}
