// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve resumable storage categories across native protection and payload wrappers.
use super::*;
use crate::{ClaimError, durable_storage::payload::TransferFailure};

/// An observed control outcome cannot become recoverable pressure or contention via its cause.
#[test]
fn original_control_keeps_priority_over_simultaneous_native_pressure_and_contention()
-> io::Result<()> {
    for control in [io::ErrorKind::Interrupted, io::ErrorKind::TimedOut] {
        for native in [32, 112] {
            let exposed =
                public_error(io::Error::new(control, io::Error::from_raw_os_error(native)));
            assert_eq!(exposed.kind(), control);
            let retained = exposed
                .get_ref()
                .and_then(|source| source.downcast_ref::<io::Error>())
                .ok_or_else(|| io::Error::other("Original native cause lost"))?;
            assert_eq!(retained.raw_os_error(), Some(native));
        }
    }
    Ok(())
}

/// Physical disk pressure remains pressure after the shared payload wraps the original I/O cause.
#[test]
fn wrapped_disk_full_retains_original_payload_cause_and_storage_category() -> io::Result<()> {
    let cause = io::Error::other(TransferFailure::Storage(io::Error::from_raw_os_error(112)));
    let exposed = public_error(cause);
    assert_eq!(exposed.kind(), io::ErrorKind::StorageFull);
    let original = exposed
        .get_ref()
        .and_then(|cause| cause.downcast_ref::<io::Error>())
        .and_then(io::Error::get_ref)
        .and_then(|cause| cause.downcast_ref::<TransferFailure>())
        .ok_or_else(|| io::Error::other("Original payload cause was lost"))?;
    assert!(
        matches!(original, TransferFailure::Storage(cause) if cause.raw_os_error() == Some(112))
    );
    for category in [io::ErrorKind::Interrupted, io::ErrorKind::TimedOut, io::ErrorKind::WouldBlock]
    {
        let exposed =
            public_error(io::Error::other(TransferFailure::Storage(io::Error::from(category))));
        assert_eq!(exposed.kind(), category);
    }
    Ok(())
}

/// A secondary native-close failure prevents classification as safely recoverable contention.
#[test]
fn cleanup_failure_keeps_both_native_causes_and_rejects_recoverable_reclassification()
-> io::Result<()> {
    let primary = ClaimError::new("fixture_primary", io::Error::from_raw_os_error(32));
    let cleanup = ClaimError::new("fixture_close", io::Error::from_raw_os_error(112));
    let joined = ClaimError::finish::<()>(Err(primary), Err(cleanup))
        .err()
        .ok_or_else(|| io::Error::other("Joined failure absent"))?;
    let exposed = public_error(io::Error::other(joined));
    assert_eq!(exposed.kind(), io::ErrorKind::Other);
    let retained = exposed
        .get_ref()
        .and_then(|cause| cause.downcast_ref::<ClaimError>())
        .ok_or_else(|| io::Error::other("Original native cause absent"))?;
    assert_eq!(retained.operation(), "fixture_primary");
    assert_eq!(retained.cleanup_cause().map(ClaimError::operation), Some("fixture_close"));
    Ok(())
}
