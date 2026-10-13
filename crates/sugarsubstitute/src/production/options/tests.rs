// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize exact option shape without filesystem discovery or native effects.
use super::*;
use std::os::windows::ffi::{OsStrExt, OsStringExt};

fn arguments() -> Vec<OsString> {
    [
        ("--startup-config", r"E:\requested\startup.local.toml"),
        ("--frontend-package", r"E:\requested\frontend"),
        ("--frontend-metadata", r"E:\requested\METADATA"),
        ("--sugarcubes-root", r"E:\requested\SugarCubes"),
        ("--backend-script", r"E:\requested\main.py"),
        ("--profile-directory", r"E:\requested\profile"),
    ]
    .into_iter()
    .flat_map(|(flag, path)| [OsString::from(flag), OsString::from(path)])
    .collect()
}

#[test]
fn all_explicit_paths_move_without_normalization() -> Result<(), Box<dyn Error>> {
    let parsed = ProductionEntryOptions::parse(arguments())?;
    for (actual, expected) in [
        (&parsed.configuration, r"E:\requested\startup.local.toml"),
        (&parsed.package, r"E:\requested\frontend"),
        (&parsed.metadata, r"E:\requested\METADATA"),
        (&parsed.cubes, r"E:\requested\SugarCubes"),
        (&parsed.script, r"E:\requested\main.py"),
        (&parsed.profile, r"E:\requested\profile"),
    ] {
        assert_eq!(actual.as_os_str(), std::ffi::OsStr::new(expected));
    }
    Ok(())
}

#[test]
fn raw_windows_units_and_owned_allocation_survive_parse() -> Result<(), Box<dyn Error>> {
    let mut input = arguments();
    let mut units: Vec<u16> = r"E:\requested\".encode_utf16().collect();
    units.extend([0xd800, 0xdc00, 0xdfff]);
    input[3] = OsString::from_wide(&units);
    let before_capacity = input[3].capacity();
    let before_allocation = input[3].as_encoded_bytes().as_ptr();
    let parsed = ProductionEntryOptions::parse(input)?;
    assert_eq!(parsed.package.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    assert_eq!(parsed.package.capacity(), before_capacity);
    assert_eq!(parsed.package.as_os_str().as_encoded_bytes().as_ptr(), before_allocation);
    Ok(())
}

#[test]
fn every_required_option_and_every_value_are_required() {
    for pair in 0..6 {
        let mut missing = arguments();
        missing.drain(pair * 2..pair * 2 + 2);
        assert!(matches!(
            ProductionEntryOptions::parse(missing),
            Err(ProductionOptionsError::Missing)
        ));
        let valueless = arguments().into_iter().take(pair * 2 + 1);
        assert!(matches!(
            ProductionEntryOptions::parse(valueless),
            Err(ProductionOptionsError::MissingValue)
        ));
    }
}

#[test]
fn duplicates_unknown_and_mixed_roles_reject() {
    for flag in ["--demo", "--internal-native-pipe-child", "--unknown", "positional"] {
        let mut input = arguments();
        input.push(flag.into());
        assert!(matches!(
            ProductionEntryOptions::parse(input),
            Err(ProductionOptionsError::Unknown)
        ));
    }
    for pair in 0..6 {
        let mut input = arguments();
        input.push(input[pair * 2].clone());
        assert!(matches!(
            ProductionEntryOptions::parse(input),
            Err(ProductionOptionsError::Duplicate)
        ));
    }
}

#[test]
fn authoritative_path_rejection_and_redacted_causes_are_preserved() -> Result<(), Box<dyn Error>> {
    for path in ["relative", r"E:\requested\..\secret", "E:\\requested\\secret\0path"] {
        let mut input = arguments();
        input[1] = path.into();
        let failure = ProductionEntryOptions::parse(input);
        let Err(cause @ ProductionOptionsError::Path(_)) = failure else {
            return Err("Invalid inert path was admitted".into());
        };
        assert!(cause.source().is_some());
        assert!(!cause.to_string().contains("secret"));
        assert!(!format!("{cause:?}").contains("secret"));
    }
    Ok(())
}

#[test]
fn aggregate_capacity_rejects_without_filesystem_effects() {
    let mut input = arguments();
    for index in [1, 3, 5, 7, 9, 11] {
        let mut path = OsString::from(r"E:\requested\");
        path.push("a".repeat(20_000));
        input[index] = path;
    }
    assert!(matches!(ProductionEntryOptions::parse(input), Err(ProductionOptionsError::Capacity)));
}
