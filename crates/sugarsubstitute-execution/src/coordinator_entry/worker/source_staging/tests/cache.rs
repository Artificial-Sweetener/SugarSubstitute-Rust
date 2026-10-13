// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound repeated original observations and converted operation storage on real admitted owners.
use super::*;
use crate::EntryPreparedCauseKind;

#[test]
fn repeated_failed_operations_share_one_first_source_and_eight_slots() -> TestResult {
    let fixture = Fixture::new()?;
    let mut first = EntryTransportError::prepared(
        EntryTransportError::Malformed,
        None,
        Some(&fixture.prepared.owner),
    );
    let EntryTransportError::PreparedRetirement(ref original) = first else {
        return Err("Missing paired operation".into());
    };
    let source = original.original().ok_or("Missing first view")? as *const _;
    for _ in 0..24 {
        let next = EntryTransportError::prepared(
            EntryTransportError::Busy,
            None,
            Some(&fixture.prepared.owner),
        );
        let EntryTransportError::PreparedRetirement(ref retained) = next else {
            return Err("Missing reused ledger".into());
        };
        assert!(std::ptr::eq(retained.original().ok_or("Missing shared view")?, source));
        first.retain_cleanup(next);
    }
    let EntryTransportError::PreparedRetirement(ref retained) = first else {
        return Err("Lost ledger".into());
    };
    assert_eq!(
        retained.original().map(crate::EntryPreparedCause::kind),
        Some(EntryPreparedCauseKind::Malformed)
    );
    assert_eq!(retained.cleanup_causes().count(), 8);
    assert_eq!(retained.omitted_cleanup(), 16);
    Ok(())
}

#[test]
fn actual_cleanup_constructor_keeps_one_retirement_layer_and_charged_conversion() -> TestResult {
    let fixture = Fixture::new()?;
    let mut first = EntryTransportError::Malformed;
    for _ in 0..24 {
        first.retain_cleanup(EntryTransportError::native(NativeObservationError::new(
            "actual_cleanup",
            std::io::Error::other("synthetic native failure"),
        )));
    }
    assert!(
        matches!(&first,EntryTransportError::Retirement{original,..} if matches!(original.as_ref(),EntryTransportError::Malformed))
    );
    let paired = EntryTransportError::prepared(first, None, Some(&fixture.prepared.owner));
    let EntryTransportError::PreparedRetirement(ref retained) = paired else {
        return Err("Missing paired source".into());
    };
    assert_eq!(
        retained.original().map(crate::EntryPreparedCause::kind),
        Some(EntryPreparedCauseKind::Retirement)
    );
    Ok(())
}
