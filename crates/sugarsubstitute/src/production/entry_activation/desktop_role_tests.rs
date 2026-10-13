// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the dedicated lexical child role without manufacturing native bootstrap authority.
use super::*;
type TestResult = Result<(), Box<dyn Error>>;
fn input() -> Vec<OsString> {
    [
        "--internal-production-desktop",
        "--bootstrap",
        "123",
        "--until-qpc",
        "90000",
        "--qpc-frequency",
        "1000",
        "--namespace",
        &"03".repeat(32),
        "--epoch",
        &"04".repeat(16),
        "--instance",
        "7",
        "--run-generation",
        "8",
        "--launch",
        "9",
        "--navigation",
        "10",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}
#[test]
fn desktop_role_preserves_original_counter_and_full_identity() -> TestResult {
    let role = DesktopRole::parse(input())?;
    assert_eq!(role.original.ticks(), 90000);
    assert_eq!(role.original.frequency(), 1000);
    assert_eq!(role.run.owner.namespace.bytes(), &[3; 32]);
    assert_eq!(role.run.owner.epoch.bytes(), &[4; 16]);
    assert_eq!(role.run.instance.get(), 7);
    assert_eq!(role.run.generation.get(), 8);
    assert_eq!(role.launch.get(), 9);
    assert_eq!(role.navigation.get(), 10);
    Ok(())
}
#[test]
fn desktop_role_rejects_zero_pseudo_negative_and_malformed_identity() {
    for (at, bad) in [
        (2, "0"),
        (2, "18446744073709551615"),
        (4, "-1"),
        (6, "0"),
        (8, "aa"),
        (10, "GG"),
        (12, "0"),
        (14, "0"),
        (16, "0"),
        (18, "0"),
    ] {
        let mut arguments = input();
        arguments[at] = bad.into();
        assert!(DesktopRole::parse(arguments).is_err());
    }
}
#[test]
fn desktop_role_rejects_extra_missing_reordered_or_foreign_role_arguments() {
    let mut extra = input();
    extra.push("--profile".into());
    assert!(DesktopRole::parse(extra).is_err());
    let mut missing = input();
    missing.pop();
    assert!(DesktopRole::parse(missing).is_err());
    let mut reordered = input();
    reordered.swap(3, 5);
    assert!(DesktopRole::parse(reordered).is_err());
    let mut foreign = input();
    foreign[0] = "--demo".into();
    assert!(DesktopRole::parse(foreign).is_err());
}
#[test]
fn desktop_role_hex_round_trip_preserves_every_byte_and_rejects_uppercase() -> TestResult {
    let original = std::array::from_fn::<_, 256, _>(|at| at as u8);
    assert_eq!(hex::<256>(&encode_hex(&original))?, original);
    assert!(hex::<1>("AF").is_err());
    assert!(hex::<1>("é").is_err());
    Ok(())
}
