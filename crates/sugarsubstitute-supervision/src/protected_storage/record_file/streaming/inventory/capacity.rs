// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replace native observation vectors only while both actual capacity claims remain live.
use super::*;

impl StreamingPort {
    /// Reserve the complete replacement before moving any original observations.
    /// Runs on the original admitted worker; this grants no external mutation authority.
    /// Old vector and retention remain unchanged on rejected/failed reservation.
    /// # Errors
    /// Rejects completed, foreign, uncharged or shrinking vectors, invalid capacity,
    /// cumulative original-capacity exhaustion, cancellation and allocation failure.
    pub(crate) fn reserve_native_observations(
        scan: &mut Scan,
        capacity: usize,
        original: &DiagnosticNativeGrant,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        original.check(context).map_err(ClaimError::diagnostic_allocation)?;
        let retained = match scan._native_retention.as_ref() {
            Some(retained) => retained.matches(original, scan.entries.capacity()),
            None => scan.entries.capacity() == 0 && scan.entries.is_empty(),
        };
        if !retained || scan.complete || capacity < scan.entries.capacity() {
            return Err(ClaimError::diagnostic_allocation(original.reject_native_state()));
        }
        let retention = original
            .scan_retention(capacity, context)
            .map_err(ClaimError::diagnostic_allocation)?;
        let mut causes = NativeCauseDestinations::diagnostic(original)?;
        let mut replacement = Vec::new();
        replacement.try_reserve_exact(capacity).map_err(|cause| {
            causes.error("diagnostic_scan_reservation", BoundedNativeCause::Reserve(cause), false)
        })?;
        if replacement.capacity() != capacity {
            return Err(ClaimError::diagnostic_allocation(original.reject_native_state()));
        }
        original.check(context).map_err(ClaimError::diagnostic_allocation)?;
        replacement.append(&mut scan.entries);
        let previous = std::mem::replace(&mut scan.entries, replacement);
        let previous_retention = scan._native_retention.replace(retention);
        drop(previous);
        drop(previous_retention);
        Ok(())
    }
}
