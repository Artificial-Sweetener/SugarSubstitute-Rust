// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The actual invocation parser rejects ambiguous or capability-bearing role shapes.
use super::*;
fn arguments() -> Vec<OsString> {
    [
        "--internal-recovery-supervisor",
        "--bootstrap",
        "64",
        "--desktop-bootstrap",
        "68",
        "--job",
        "72",
        "--until-qpc",
        "100",
        "--qpc-frequency",
        "10",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}
#[test]
fn exact_distinct_inherited_slots_and_original_counter_are_accepted() -> Result<(), Box<dyn Error>>
{
    let role = Role::parse(arguments())?;
    assert!(role.input.is_some());
    assert!(role.job.is_some());
    assert_eq!(role.original.ticks(), 100);
    assert_eq!(role.original.frequency(), 10);
    Ok(())
}
#[test]
fn replay_alias_pseudo_zero_overflow_and_noncanonical_fields_reject() {
    for index in [2, 4, 6, 8, 10] {
        for value in ["0", "00", "064", "+64", "-64", "64 ", " 64", "18446744073709551616", "糖"] {
            let mut input = arguments();
            input[index] = value.into();
            assert!(Role::parse(input).is_err(), "index={index}, value={value}");
        }
    }
    for value in ["64", "18446744073709551615", "18446744073709551614"] {
        let mut input = arguments();
        input[4] = value.into();
        assert!(Role::parse(input).is_err());
    }
}
#[test]
fn wrong_order_missing_fields_and_unrecognized_material_reject() {
    let canonical = arguments();
    for length in 0..canonical.len() {
        assert!(Role::parse(canonical[..length].to_vec()).is_err());
    }
    let mut swapped = arguments();
    swapped.swap(1, 3);
    assert!(Role::parse(swapped).is_err());
    for extra in ["--reset", "--capability", "--installation", "unexpected"] {
        let mut input = arguments();
        input.push(extra.into());
        assert!(Role::parse(input).is_err());
    }
}

#[test]
fn job_cannot_alias_either_inherited_section() {
    for value in ["64", "68"] {
        let mut input = arguments();
        input[6] = value.into();
        assert!(Role::parse(input).is_err());
    }
}

#[test]
fn producer_registry_round_trips_exact_child_field_order() -> Result<(), Box<dyn Error>> {
    use std::os::windows::ffi::OsStringExt;
    let fields = super::encode([64, 68, 72, 100, 10])
        .map_err(|cause| std::io::Error::other(cause.to_string()))?;
    let produced: Vec<OsString> = fields.iter().map(|field| OsString::from_wide(field)).collect();
    assert_eq!(produced, arguments());
    Role::parse(produced)?;
    assert!(matches_prefix(OsStr::new("--internal-recovery-supervisor")));
    assert!(!matches_prefix(OsStr::new("--internal-production-desktop")));
    Ok(())
}
