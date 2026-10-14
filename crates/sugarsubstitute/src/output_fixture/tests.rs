// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject invalid source roles and missing independent actual evidence without native effects.
use super::*;
use channel::{Message, decode, encode};
use options::Role;
use std::ffi::OsString;
fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}
#[test]
fn output_roles_reject_extra_reordered_and_unrecognized_bootstrap_input() {
    for values in [
        vec!["--case", "prelude"],
        vec!["--case", "corpus", "--section", "1"],
        vec!["--section", "1", "--case", "flood"],
        vec!["--case", "leaf", "extra"],
        vec!["--case", "flood"],
    ] {
        assert!(ChildOptions::parse(args(&values)).is_err());
    }
    assert!(ChildOptions::parse(args(&["--case", "corpus"])).is_ok());
    assert!(ChildOptions::parse(args(&["--case", "leaf"])).is_ok());
}
#[test]
fn output_roles_validate_original_positive_bounded_native_deadline_facts() {
    let good =
        ["--case", "flood", "--section", "1", "--deadline-qpc", "100", "--qpc-frequency", "10"];
    assert!(ChildOptions::parse(args(&good)).is_ok());
    for bad in ["0", "01", "-1", "1.0", "184467440737095516160", "９"] {
        let mut values = good;
        values[5] = bad;
        assert!(ChildOptions::parse(args(&values)).is_err());
    }
}
#[test]
fn output_protocol_rejects_other_role_version_size_opcode_zero_and_unexpected_identity()
-> Result<()> {
    let wire = encode(Role::Flood, Message::Go, 1, None);
    assert_eq!(decode(Role::Flood, &wire)?.0, Message::Go);
    assert!(decode(Role::Idle, &wire).is_err());
    assert!(decode(Role::Flood, &wire[..25]).is_err());
    for (index, value) in [(0, b'X'), (4, 99), (5, Role::Idle as u8), (6, 0), (14, 1)] {
        let mut bad = wire;
        bad[index] = value;
        assert!(decode(Role::Flood, &bad).is_err());
    }
    Ok(())
}
#[test]
fn diagnostic_error_reply_requires_exact_idle_role_positive_code_and_zero_reserved_birth()
-> Result<()> {
    let wire = encode(Role::Idle, Message::DiagnosticFailed, 2, Some((232, 0)));
    assert_eq!(decode(Role::Idle, &wire)?.2, Some((232, 0)));
    assert!(decode(Role::Flood, &wire).is_err());
    assert!(decode(Role::Idle, &encode(Role::Idle, Message::DiagnosticFailed, 2, None)).is_err());
    assert!(
        decode(Role::Idle, &encode(Role::Idle, Message::DiagnosticFailed, 2, Some((232, 9))))
            .is_err()
    );
    Ok(())
}
#[test]
fn output_forward_identity_is_exact_positive_and_only_in_actual_done() -> Result<()> {
    let wire = encode(Role::Forward, Message::Done, 2, Some((37, 259)));
    assert_eq!(decode(Role::Forward, &wire)?.2, Some((37, 259)));
    assert!(decode(Role::Forward, &encode(Role::Forward, Message::Done, 2, None)).is_err());
    assert!(
        decode(Role::Forward, &encode(Role::Forward, Message::Go, 1, Some((37, 259)))).is_err()
    );
    Ok(())
}
#[test]
fn source_write_splits_preserve_independently_frozen_unicode_ansi_and_cr_bytes() {
    assert_eq!(corpus::OUT_PARTS.concat(), corpus::OUT_EXPECTED);
    assert_eq!(corpus::ERR_PARTS.concat(), corpus::ERR_EXPECTED);
    assert!(corpus::OUT_PARTS.iter().any(|bytes| std::str::from_utf8(bytes).is_err()));
    assert!(corpus::ERR_PARTS.iter().any(|bytes| std::str::from_utf8(bytes).is_err()));
    const { assert!(corpus::FLOOD_BYTES > 2 * 1024 * 1024) };
}
#[test]
fn original_cleanup_budget_is_fixed_ten_and_fifteen_seconds_without_reset() -> Result<()> {
    use sugarsubstitute_contracts::{execution::Moment, lifecycle::ShutdownId};
    let stop = evidence::Stop::new(
        ShutdownId::new(1)?,
        Moment::from_nanos(100),
        std::time::Instant::now(),
    )?;
    assert_eq!(stop.budget.graceful_until.as_nanos(), 10_000_000_100);
    assert_eq!(stop.budget.settle_until.as_nanos(), 15_000_000_100);
    assert_eq!(stop.next(), Some(stop.grace));
    Ok(())
}
