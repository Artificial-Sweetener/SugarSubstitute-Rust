// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual original source projections and final preparation credit through the raw ledger.
use super::*;
use crate::EntryPreparedCauseKind;
use std::{error::Error, fmt, sync::atomic::AtomicBool};

struct OriginalCause {
    memory: MemoryLease,
    dropped_while_claimed: Arc<AtomicBool>,
}
impl fmt::Debug for OriginalCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OriginalCause")
    }
}
impl fmt::Display for OriginalCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Controlled original native rejection")
    }
}
impl Error for OriginalCause {}
impl Drop for OriginalCause {
    fn drop(&mut self) {
        self.dropped_while_claimed.store(self.memory.claim(1).is_err(), Ordering::Release);
    }
}

#[test]
fn raw_native_causes_preserve_exact_source_projection_flags_and_final_original_credit() -> TestResult
{
    for variant in 0..5 {
        let fixture = Fixture::new()?;
        let core = fixture.core.clone();
        let scope = fixture.original.id();
        let dropped = Arc::new(AtomicBool::new(false));
        let native = NativeObservationError::new(
            "original_raw_source",
            OriginalCause {
                memory: fixture.prepared.owner.original_context().memory().clone(),
                dropped_while_claimed: dropped.clone(),
            },
        );
        let original = match variant {
            0 => EntryTransportError::Clock(native),
            1 => EntryTransportError::PeerDisconnected(native),
            2 => EntryTransportError::EndpointAbsent(native),
            _ => EntryTransportError::Native { cause: native, identity_unrestored: variant == 4 },
        };
        let error = EntryTransportError::prepared(original, None, Some(&fixture.prepared.owner));
        let EntryTransportError::PreparedRetirement(ref retained) = error else {
            return Err("Genuine prepared ledger missing".into());
        };
        let source = retained.original().ok_or("Original source missing")?;
        let kind = match variant {
            0 => EntryPreparedCauseKind::Clock,
            1 | 2 => EntryPreparedCauseKind::Retirement,
            _ => EntryPreparedCauseKind::Native,
        };
        assert_eq!(source.kind(), kind);
        assert_eq!(source.native_stage(), Some("original_raw_source"));
        assert_eq!(error.requires_process_shutdown(), variant == 4);
        let primary = source.source().ok_or("Original primary missing")?;
        if variant == 1 || variant == 2 {
            let native = primary
                .downcast_ref::<NativeObservationError>()
                .ok_or("Native source projection changed")?;
            assert_eq!(native.stage(), "original_raw_source");
            assert!(native.source().is_some_and(|cause| cause.is::<OriginalCause>()));
        } else {
            assert!(primary.is::<OriginalCause>());
        }
        let last = error.clone();
        drop(fixture);
        assert_eq!(core.scope_snapshot(scope).reserved_bytes, 4 * 1024 * 1024);
        assert!(!dropped.load(Ordering::Acquire));
        drop(error);
        assert_eq!(core.scope_snapshot(scope).reserved_bytes, 4 * 1024 * 1024);
        assert!(!dropped.load(Ordering::Acquire));
        drop(last);
        assert!(dropped.load(Ordering::Acquire));
        assert_eq!(core.scope_snapshot(scope).reserved_bytes, 0);
    }
    Ok(())
}

#[test]
fn raw_inert_shape_and_retirement_sources_remain_typed_under_actual_preparation_owner() -> TestResult
{
    use sugarsubstitute_contracts::coordinator_entry::EntryShapeError;
    let cases = [
        (EntryTransportError::Closed, EntryPreparedCauseKind::Closed),
        (EntryTransportError::Busy, EntryPreparedCauseKind::Busy),
        (EntryTransportError::InvalidOperation, EntryPreparedCauseKind::InvalidOperation),
        (EntryTransportError::InvalidDeadline, EntryPreparedCauseKind::InvalidDeadline),
        (EntryTransportError::InvalidPhase, EntryPreparedCauseKind::InvalidPhase),
        (EntryTransportError::Authentication, EntryPreparedCauseKind::Authentication),
        (EntryTransportError::Malformed, EntryPreparedCauseKind::Malformed),
        (EntryTransportError::Capacity, EntryPreparedCauseKind::Capacity),
        (EntryTransportError::Shape(EntryShapeError::Path), EntryPreparedCauseKind::Shape),
    ];
    for (original, kind) in cases {
        let fixture = Fixture::new()?;
        let error = EntryTransportError::prepared(original, None, Some(&fixture.prepared.owner));
        let EntryTransportError::PreparedRetirement(ref retained) = error else {
            return Err("Genuine prepared ledger missing".into());
        };
        let source = retained.original().ok_or("Original source missing")?;
        assert_eq!(source.kind(), kind);
        if kind == EntryPreparedCauseKind::Shape {
            assert_eq!(
                source.source().and_then(|cause| cause.downcast_ref::<EntryShapeError>()),
                Some(&EntryShapeError::Path)
            );
        } else {
            assert!(source.source().is_none());
        }
        assert!(!error.requires_process_shutdown());
    }
    let fixture = Fixture::new()?;
    let original = EntryTransportError::Retirement {
        original: Box::new(EntryTransportError::Shape(EntryShapeError::Arguments)),
        cleanup: NativeObservationError::new(
            "independent_cleanup",
            std::io::Error::other("Controlled cleanup failure"),
        ),
        identity_unrestored: true,
    };
    let error = EntryTransportError::prepared(original, None, Some(&fixture.prepared.owner));
    let EntryTransportError::PreparedRetirement(ref retained) = error else {
        return Err("Genuine prepared retirement ledger missing".into());
    };
    let source = retained.original().ok_or("Original retirement missing")?;
    assert_eq!(source.kind(), EntryPreparedCauseKind::Retirement);
    assert_eq!(source.native_stage(), Some("independent_cleanup"));
    let primary = source
        .source()
        .and_then(|cause| cause.downcast_ref::<crate::EntryPreparedCause>())
        .ok_or("Original typed retirement primary missing")?;
    assert_eq!(primary.kind(), EntryPreparedCauseKind::Shape);
    assert_eq!(
        primary.source().and_then(|cause| cause.downcast_ref::<EntryShapeError>()),
        Some(&EntryShapeError::Arguments)
    );
    assert!(error.requires_process_shutdown());
    Ok(())
}
