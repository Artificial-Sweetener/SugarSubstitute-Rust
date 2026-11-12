// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume only already-issued profile receipts, independently of fresh setup admission.
use crate::{BackendCustodyAcknowledgement, BackendCustodyParent, ClaimError};
use sugarsubstitute_contracts::backend_custody::{BackendCustodyReceipt, ProfileCustodyKey};

/// Retain the original same-source response before validating its full association.
/// No nonce, publication, counter cutoff or native effect is created here.
/// # Errors
/// Rejects changed source/key/nonce/purpose while retaining the complete received guard.
pub(in crate::profiles) fn published(
    parent: &BackendCustodyParent,
    key: ProfileCustodyKey,
    nonce: Option<u64>,
    acknowledged: &mut Option<BackendCustodyAcknowledgement>,
) -> Result<(), ClaimError> {
    if observe(parent, BackendCustodyReceipt::ProfilePublished(key), nonce, acknowledged)? {
        tracing::info!(operation="profile_published_acknowledged",profile=?key,nonce,
            "Actual helper validated the protected prospective Held row");
    }
    Ok(())
}

/// Retain a previously issued Prepared response without admitting setup or clearing outputs.
/// The destination's original remote cells remain until the genuine Released receipt closes them.
/// # Errors
/// Rejects changed source/key/nonce/purpose while retaining the full received guard.
pub(in crate::profiles) fn prepared_during_close(
    parent: &BackendCustodyParent,
    key: ProfileCustodyKey,
    nonce: Option<u64>,
    closing: &std::sync::atomic::AtomicBool,
    acknowledged: &mut Option<BackendCustodyAcknowledgement>,
) -> Result<(), ClaimError> {
    if !closing.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(());
    }
    observe(parent, BackendCustodyReceipt::ProfilePrepared(key), nonce, acknowledged).map(|_| ())
}

/// Remove only the matching cached response and preserve its source before validation.
fn observe(
    parent: &BackendCustodyParent,
    receipt: BackendCustodyReceipt,
    nonce: Option<u64>,
    acknowledged: &mut Option<BackendCustodyAcknowledgement>,
) -> Result<bool, ClaimError> {
    let Some(nonce) = nonce else {
        return if acknowledged.is_none() {
            Ok(false)
        } else {
            Err(ClaimError::invalid("profile_acknowledgement_missing_nonce"))
        };
    };
    let first = acknowledged.is_none();
    if first {
        *acknowledged = parent.take_acknowledgement_for(nonce);
    }
    let Some(ack) = acknowledged else { return Ok(false) };
    if !ack.matches_storage(parent.storage())
        || ack.key() != parent.key()
        || ack.nonce() != nonce
        || ack.receipt() != receipt
    {
        return Err(ClaimError::invalid("profile_acknowledgement_association"));
    }
    Ok(first)
}
