// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preflight the actual simultaneous data/control graph before any Sup allocation.
use super::super::names::{CaptureNativeSid, name_control_bytes, transient_control_bytes};
use super::*;
use sugarsubstitute_execution::CaptureBundleOrigin;
fn add(a: usize, b: usize) -> Result<usize, CaptureCapacityFailure> {
    a.checked_add(b).ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Overflow))
}
/// All layouts are compiled source-owner types; input supplies no numerical capacity.
pub(super) fn regions(
    origin: &crate::installed_release::ReleaseAuthority,
) -> Result<(usize, usize, usize), CaptureCapacityFailure> {
    let file = NativeFile::diagnostic_owner_layout()
        .map_err(|_| CaptureCapacityFailure::new(CapacityFailureKind::Layout))?
        .retained;
    let ticket = SharedCaptureLoan::control_bytes();
    let mut metadata = add(origin.control_storage_bytes(), AllocationOrigin::control_bytes())?;
    for bytes in [
        size_of::<Fields>(),
        ticket,
        CaptureOutputQuota::control_storage_bytes(),
        ticket,
        ticket,
        8 * (file + ticket),
        8 * (name_control_bytes() + ticket),
        72 + CaptureNativeSid::control_bytes() + ticket,
        transient_control_bytes(),
    ] {
        metadata = add(metadata, bytes)?;
    }
    let cause = ClaimError::capture_native_cause_storage_bytes()
        .ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Overflow))?;
    let ingress = add(
        65_536 + 384,
        CAUSE_SLOTS
            .checked_mul(cause)
            .ok_or(CaptureCapacityFailure::new(CapacityFailureKind::Overflow))?,
    )?;
    // Actual canonical decode has one raw UTF16 vector and one WTF8 destination;
    // producing native path plus bounded security descriptor are a separate phase.
    let path_peak = 65_534_usize + 98_301 + size_of::<Vec<u16>>() + size_of::<std::path::PathBuf>();
    let security = crate::protected_storage::security::diagnostic_security_layout()
        .map_err(|_| CaptureCapacityFailure::new(CapacityFailureKind::Layout))?
        .peak;
    let hash = NativeFile::diagnostic_hash_layout()
        .map_err(|_| CaptureCapacityFailure::new(CapacityFailureKind::Layout))?
        .peak;
    let account = crate::protected_storage::account::diagnostic_account_layout()
        .map_err(|_| CaptureCapacityFailure::new(CapacityFailureKind::Layout))?
        .peak;
    let scratch = [path_peak, 65_534 + security, hash, account]
        .into_iter()
        .max()
        .ok_or(CaptureCapacityFailure::missing())?;
    Ok((metadata, ingress, scratch))
}

pub(super) fn preflight(origin: &CaptureBundleOrigin) -> Result<(), CaptureCapacityFailure> {
    preflight_authority(&crate::installed_release::ReleaseAuthority::Capture(origin.clone()))
}

pub(super) fn preflight_authority(
    origin: &crate::installed_release::ReleaseAuthority,
) -> Result<(), CaptureCapacityFailure> {
    let (metadata, ingress, scratch) = regions(origin)?;
    if metadata > METADATA_BYTES || ingress > INGRESS_BYTES || scratch > SCRATCH_BYTES {
        return Err(CaptureCapacityFailure::new(CapacityFailureKind::Layout));
    }
    Ok(())
}
