// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove strict role-order parsing has no native ownership/effects.
use super::*;
use std::{ffi::OsString, os::windows::ffi::OsStringExt};

fn arguments(code: &str) -> Vec<OsString> {
    [
        "--ready",
        "4",
        "--release",
        "8",
        "--challenge",
        "12",
        "--response",
        "16",
        "--deadline-qpc",
        "1000000",
        "--qpc-frequency",
        "10000000",
        "--correlation",
        "7",
        "--exit-code",
        code,
    ]
    .map(OsString::from)
    .into()
}

#[test]
fn options_accept_three_exact_exit_codes_and_preserve_original_deadline() -> Result<()> {
    for (code, expected) in [("0", 0), ("37", 37), ("259", 259)] {
        let options = ChildOptions::parse(arguments(code))?;
        assert_eq!(options.exit_code, expected);
        assert_eq!(options.deadline.ticks(), 1_000_000);
        assert_eq!(options.deadline.frequency(), 10_000_000);
        assert_eq!(options.correlation.get(), 7);
        assert!(options.events.is_some());
    }
    Ok(())
}

#[test]
fn missing_duplicate_reordered_and_extra_pairs_reject() {
    let full = arguments("0");
    for count in 0..full.len() {
        assert!(ChildOptions::parse(full[..count].iter().cloned()).is_err())
    }
    let mut duplicate = full.clone();
    duplicate[2] = "--ready".into();
    assert!(ChildOptions::parse(duplicate).is_err());
    let mut reordered = full.clone();
    reordered.swap(0, 2);
    reordered.swap(1, 3);
    assert!(ChildOptions::parse(reordered).is_err());
    let mut extra = full;
    extra.extend(["--exit-code".into(), "37".into()]);
    assert!(ChildOptions::parse(extra).is_err());
}

#[test]
fn numeric_tokens_reject_signs_zero_nonascii_leading_zeroes_and_overflow() {
    for index in [1, 3, 5, 7, 9, 11, 13] {
        for value in [
            "",
            "0",
            "+1",
            "-1",
            "01",
            " 1",
            "1 ",
            "１",
            "١",
            "1e3",
            "18446744073709551616",
            "111111111111111111111",
        ] {
            let mut input = arguments("0");
            input[index] = value.into();
            assert!(ChildOptions::parse(input).is_err());
        }
    }
}

#[test]
fn event_duplicates_pseudo_handles_and_signed_qpc_overflow_reject() {
    for index in [3, 5, 7] {
        let mut input = arguments("0");
        input[index] = "4".into();
        assert!(ChildOptions::parse(input).is_err());
    }
    for value in [u64::MAX, (usize::MAX - 15) as u64] {
        let mut input = arguments("0");
        input[1] = value.to_string().into();
        assert!(ChildOptions::parse(input).is_err());
    }
    for index in [9, 11] {
        let mut input = arguments("0");
        input[index] = ((i64::MAX as u64) + 1).to_string().into();
        assert!(ChildOptions::parse(input).is_err());
    }
}

#[test]
fn nonunicode_flags_values_and_unsupported_exit_codes_reject_redacted() -> Result<()> {
    for index in [0, 1, 14, 15] {
        let mut input = arguments("0");
        input[index] = OsString::from_wide(&[0xd800]);
        assert!(ChildOptions::parse(input).is_err());
    }
    for code in ["1", "-1", "+0", "0259", "３７", "259 "] {
        assert!(ChildOptions::parse(arguments(code)).is_err());
    }
    let mut input = arguments("0");
    input[1] = "PRIVATE_INVALID_HANDLE_TOKEN".into();
    let error = ChildOptions::parse(input).err().ok_or("Missing parser rejection")?;
    assert!(!error.to_string().contains("PRIVATE_INVALID"));
    assert!(!format!("{error:?}").contains("PRIVATE_INVALID"));
    Ok(())
}

#[test]
fn correlation_full_u64_and_qpc_signed_boundary_are_preserved_without_effects() -> Result<()> {
    let mut input = arguments("259");
    input[9] = i64::MAX.to_string().into();
    input[11] = i64::MAX.to_string().into();
    input[13] = u64::MAX.to_string().into();
    let options = ChildOptions::parse(input)?;
    assert_eq!(options.deadline.ticks(), i64::MAX as u64);
    assert_eq!(options.deadline.frequency(), i64::MAX as u64);
    assert_eq!(options.correlation.get(), u64::MAX);
    Ok(())
}
