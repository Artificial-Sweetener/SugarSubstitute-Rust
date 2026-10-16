// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject malformed observational bytes without giving them readiness authority.
use super::*;
const LINE: &str = "INFO sugarsubstitute_desktop::frontend_probe_diagnostic: operation=\"frontend_probe_diagnostic\" schema=1 stage=Incomplete launch=1 navigation=2 probe_operation=3 issued=3 completed=3 blocked=2048";

#[test]
fn closed_incomplete_mask_accepts_only_known_nonempty_conditions() {
    for bit in 0..15 {
        assert_eq!(Blocked::new(1 << bit).map(Blocked::bits), Some(1 << bit));
    }
    assert_eq!(Blocked::new(32767).map(Blocked::bits), Some(32767));
    for unknown in [0, 32768, u16::MAX] {
        assert!(Blocked::new(unknown).is_none());
    }
}

#[test]
fn probe_record_rejects_noncanonical_fields_and_arbitrary_payloads() {
    assert_eq!(Record::parse(LINE.as_bytes()).map(|record| record.blocked), Some(2048));
    for (from, to) in [
        ("schema=1", "schema=2"),
        ("launch=1", "launch=0"),
        ("launch=1", "launch=01"),
        ("launch=1", "launch=+1"),
        ("navigation=2", "navigation=0"),
        ("blocked=2048", "blocked=32768"),
        ("blocked=2048", "blocked=0"),
        ("completed=3", "completed=4"),
        ("completed=3", "completed=1"),
        ("probe_operation=3", "probe_operation=2"),
        ("stage=Incomplete", "stage=Unknown"),
        ("issued=3", "issued=18446744073709551616"),
        ("sugarsubstitute_desktop::frontend_probe_diagnostic:", "other:"),
    ] {
        assert!(Record::parse(LINE.replace(from, to).as_bytes()).is_none(), "accepted {to}");
    }
    for suffix in [" blocked=1", " raw=private", " message=anything"] {
        assert!(Record::parse(format!("{LINE}{suffix}").as_bytes()).is_none());
    }
    assert!(Record::parse(&[b'x'; 513]).is_none());
}

#[test]
fn timeout_record_preserves_zero_probe_and_pending_or_incomplete_custody() {
    let timeout = LINE.replace("stage=Incomplete", "stage=Timeout");
    assert!(Record::parse(timeout.as_bytes()).is_some());
    assert!(Record::parse(timeout.replace("completed=3", "completed=2").as_bytes()).is_some());
    let empty = timeout.replace(
        "probe_operation=3 issued=3 completed=3 blocked=2048",
        "probe_operation=0 issued=0 completed=0 blocked=0",
    );
    assert!(Record::parse(empty.as_bytes()).is_some());
    assert!(Record::parse(empty.replace("blocked=0", "blocked=1").as_bytes()).is_none());
}
