// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize exact option shape without filesystem discovery or native effects.
use super::*;
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};

fn root() -> PathBuf {
    PathBuf::from(if cfg!(windows) { r"E:/requested" } else { "/requested" })
}
fn arguments() -> Vec<OsString> {
    [
        ("--startup-config", "startup.local.toml"),
        ("--frontend-package", "frontend"),
        ("--frontend-metadata", "METADATA"),
        ("--sugarcubes-root", "SugarCubes"),
        ("--backend-script", "main.py"),
        ("--profile-directory", "profile"),
    ]
    .into_iter()
    .flat_map(|(flag, leaf)| [OsString::from(flag), root().join(leaf).into_os_string()])
    .collect()
}

#[test]
fn all_explicit_paths_move_without_normalization() -> Result<(), Box<dyn Error>> {
    let parsed = StartupPaths::parse(arguments())?;
    for (actual, expected) in [
        (&parsed.configuration, root().join("startup.local.toml")),
        (&parsed.package, root().join("frontend")),
        (&parsed.metadata, root().join("METADATA")),
        (&parsed.cubes, root().join("SugarCubes")),
        (&parsed.script, root().join("main.py")),
        (&parsed.profile, root().join("profile")),
    ] {
        assert_eq!(actual.as_os_str(), expected.as_os_str());
    }
    Ok(())
}

#[cfg(windows)]
#[test]
fn raw_windows_units_and_owned_allocation_survive_parse() -> Result<(), Box<dyn Error>> {
    let mut input = arguments();
    let mut units: Vec<u16> = r"E:\requested\".encode_utf16().collect();
    units.extend([0xd800, 0xdc00, 0xdfff]);
    input[3] = OsString::from_wide(&units);
    let before_capacity = input[3].capacity();
    let before_allocation = input[3].as_encoded_bytes().as_ptr();
    let parsed = StartupPaths::parse(input)?;
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
        assert!(matches!(StartupPaths::parse(missing), Err(StartupPathsError::Missing)));
        let valueless = arguments().into_iter().take(pair * 2 + 1);
        assert!(matches!(StartupPaths::parse(valueless), Err(StartupPathsError::MissingValue)));
    }
}

#[test]
fn duplicates_unknown_and_mixed_roles_reject() {
    for flag in ["--demo", "--internal-native-pipe-child", "--unknown", "positional"] {
        let mut input = arguments();
        input.push(flag.into());
        assert!(matches!(StartupPaths::parse(input), Err(StartupPathsError::Unknown)));
    }
    for pair in 0..6 {
        let mut input = arguments();
        input.push(input[pair * 2].clone());
        assert!(matches!(StartupPaths::parse(input), Err(StartupPathsError::Duplicate)));
    }
}

#[test]
fn authoritative_path_rejection_and_redacted_causes_are_preserved() -> Result<(), Box<dyn Error>> {
    for path in [
        "relative".to_owned(),
        root().join("../secret").to_string_lossy().into_owned(),
        format!("{}\0secret", root().display()),
    ] {
        let mut input = arguments();
        input[1] = path.into();
        let failure = StartupPaths::parse(input);
        let Err(cause @ StartupPathsError::Path(_)) = failure else {
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
        let mut path = root().join("a").into_os_string();
        path.push("a".repeat(20_000));
        input[index] = path;
    }
    assert!(matches!(StartupPaths::parse(input), Err(StartupPathsError::Capacity)));
}
