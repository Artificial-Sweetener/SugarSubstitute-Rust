// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject role/path/identity ambiguity before any native or filesystem effects.
use super::*;
#[test]
fn campaign_paths_require_owned_d_artifacts_and_exact_order() -> Result<()> {
    let valid = [
        "--workspace",
        r"D:\SugarSubstitute-Rust\artifacts\m1\fresh",
        "--receipt",
        r"D:\SugarSubstitute-Rust\artifacts\m1\receipt.json",
    ];
    assert!(CampaignOptions::parse(valid.map(OsString::from)).is_ok());
    for bad in
        [r"C:\private", r"D:\other", r"D:\SugarSubstitute-Rust\artifacts\m1\..\outside", "relative"]
    {
        let mut input = valid.map(OsString::from);
        input[1] = bad.into();
        assert!(CampaignOptions::parse(input).is_err());
    }
    assert!(CampaignOptions::parse(valid.into_iter().rev().map(OsString::from)).is_err());
    Ok(())
}
#[test]
fn matching_keys_and_numbers_reject_noncanonical_or_oversized_input() {
    assert_eq!(hex::<2>(OsStr::new("0aff")).ok(), Some([10, 255]));
    for value in ["0AFF", "0aff0", "", "private"] {
        assert!(hex::<2>(OsStr::new(value)).is_err());
    }
    for value in ["0", "01", "+1", "-1", "1.0", "18446744073709551616"] {
        assert!(positive(OsStr::new(value)).is_err());
    }
    assert_eq!(positive(OsStr::new("259")).ok(), Some(259));
}
fn child_arguments(role: &str) -> Vec<OsString> {
    [
        "--role",
        role,
        "--installation",
        r"D:\SugarSubstitute-Rust\artifacts\m1\fresh\installation-a",
        "--backend",
        r"D:\SugarSubstitute-Rust\artifacts\m1\fresh\backend",
        "--receipt",
        r"D:\SugarSubstitute-Rust\artifacts\m1\fresh\child.json",
        "--namespace",
        &"ab".repeat(32),
        "--epoch",
        &"cd".repeat(16),
        "--ready",
        "11",
        "--release",
        "12",
        "--challenge",
        "13",
        "--response",
        "14",
        "--deadline-qpc",
        "200",
        "--qpc-frequency",
        "100",
        "--correlation",
        "7",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}
#[test]
fn child_roles_preserve_original_matching_inputs_without_native_effects() -> Result<()> {
    for role in [
        Role::CoordinatorContender,
        Role::BackendContender,
        Role::BackendSuccess,
        Role::ActiveHolder,
        Role::ActivePredecessor,
    ] {
        let options = ChildOptions::parse(child_arguments(role.wire()))?;
        assert_eq!(options.role, role);
        assert_eq!(options.deadline.ticks(), 200);
        assert_eq!(options.deadline.frequency(), 100);
        assert_eq!(options.correlation.get(), 7);
        assert_eq!(options.parent.namespace.bytes(), &[0xab; 32]);
        assert_eq!(options.parent.epoch.bytes(), &[0xcd; 16]);
        assert!(options.events.is_some());
    }
    Ok(())
}
#[test]
fn child_parser_rejects_duplicate_handles_extra_fields_and_reserved_identity() {
    let base = child_arguments(Role::BackendContender.wire());
    for (index, value) in
        [(15, "11"), (11, "00000000000000000000000000000000"), (1, "unknown"), (25, "0")]
    {
        let mut arguments = base.clone();
        arguments[index] = value.into();
        assert!(ChildOptions::parse(arguments).is_err());
    }
    let mut extra = base.clone();
    extra.push("--light".into());
    assert!(ChildOptions::parse(extra).is_err());
    assert!(ChildOptions::parse(base.into_iter().take(24)).is_err());
}
