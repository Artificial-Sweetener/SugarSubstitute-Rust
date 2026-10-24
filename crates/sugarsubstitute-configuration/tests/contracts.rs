// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify public configuration contracts without an ambient process or filesystem.
use std::{error::Error, path::PathBuf};
use sugarsubstitute_configuration::{
    descriptor::LaunchDescriptor,
    initial,
    paths::StartupPaths,
    startup::{MAX_STARTUP_CONFIGURATION_BYTES, parse_startup_configuration},
};
use sugarsubstitute_contracts::configuration::{ConnectionConfiguration, Theme};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn root() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"E:\糖 café 🙂")
    } else {
        PathBuf::from("/糖 café 🙂")
    }
}
fn paths() -> Result<StartupPaths> {
    let root = root();
    Ok(StartupPaths::new([
        root.join("startup.local.toml"),
        root.join("frontend"),
        root.join("METADATA"),
        root.join("SugarCubes"),
        root.join("main.py"),
        root.join("profile"),
    ])?)
}

#[test]
fn initial_defaults_roundtrip_through_the_actual_runtime_parser() -> Result {
    let root = root();
    let document = initial::managed(&root, &root.join("python.exe"), &root.join("输出"), "ja")?;
    let parsed = parse_startup_configuration(document.as_bytes())?;
    assert_eq!(parsed.profile().as_str(), "default");
    assert_eq!(parsed.presentation().theme(), Theme::Dark);
    assert_eq!(parsed.presentation().locale().as_str(), "ja");
    assert!(!parsed.presentation().reduced_motion());
    let ConnectionConfiguration::Launch(launch) = parsed.connection() else {
        return Err("Expected managed launch".into());
    };
    assert_eq!(launch.root(), root);
    assert_eq!(launch.port().get(), 8188);
    assert_eq!(
        launch.arguments(),
        ["--output-directory", root.join("输出").to_str().ok_or("Unicode")?]
    );
    Ok(())
}

#[test]
fn persisted_explicit_values_win_over_initial_defaults() -> Result {
    let root = root();
    let initial = initial::managed(&root, &root.join("python.exe"), &root.join("output"), "en")?;
    let changed = initial
        .replace("8188", "9191")
        .replace("default", "custom")
        .replace("dark", "light")
        .replace("false", "true");
    let parsed = parse_startup_configuration(changed.as_bytes())?;
    assert_eq!(parsed.profile().as_str(), "custom");
    assert_eq!(parsed.presentation().theme(), Theme::Light);
    assert!(parsed.presentation().reduced_motion());
    let ConnectionConfiguration::Launch(launch) = parsed.connection() else {
        return Err("Expected managed launch".into());
    };
    assert_eq!(launch.port().get(), 9191);
    assert!(parse_startup_configuration(changed.replace("port = 9191\n", "").as_bytes()).is_err());
    Ok(())
}

#[test]
fn initial_values_cannot_bypass_semantic_validation() {
    let root = root();
    assert!(
        initial::managed(&root, &root.join("python.exe"), &root.join("output"), "unknown").is_err()
    );
    assert!(
        initial::managed(
            &PathBuf::from("relative"),
            &root.join("python.exe"),
            &root.join("output"),
            "en"
        )
        .is_err()
    );
}

#[test]
fn installed_and_explicit_inputs_resolve_identically_after_restart() -> Result {
    let expected = paths()?.into_paths();
    let bytes = LaunchDescriptor::encode(paths()?)?;
    let first = LaunchDescriptor::parse(&bytes)?.into_startup_paths()?.into_paths();
    let restarted = LaunchDescriptor::parse(&bytes)?.into_startup_paths()?.into_paths();
    assert_eq!(first, expected);
    assert_eq!(restarted, first);
    Ok(())
}

#[test]
fn maintenance_relocates_only_configuration_and_keeps_external_inputs() -> Result {
    let descriptor = LaunchDescriptor::parse(&LaunchDescriptor::encode(paths()?)?)?;
    let destination = root().join("new generation/startup.local.toml");
    let relocated = descriptor.relocated(&destination)?;
    let after = LaunchDescriptor::parse(&relocated.bytes()?)?.into_startup_paths()?.into_paths();
    let before = paths()?.into_paths();
    assert_eq!(after[0], destination);
    assert_eq!(after[1..], before[1..]);
    assert!(relocated.same_arguments(&descriptor.relocated(&destination)?));
    Ok(())
}

#[test]
fn descriptor_rejects_unknown_duplicate_malformed_and_oversized_input() -> Result {
    for bytes in [
        b"{bad".to_vec(),
        b"{\"schema_version\":2,\"arguments\":[]}".to_vec(),
        b"{\"schema_version\":1,\"arguments\":[],\"extra\":true}".to_vec(),
        b"{\"schema_version\":1,\"schema_version\":1,\"arguments\":[]}".to_vec(),
        vec![b' '; MAX_STARTUP_CONFIGURATION_BYTES + 1],
        vec![0xff],
    ] {
        assert!(LaunchDescriptor::parse(&bytes).is_err());
    }
    for arguments in [vec![], vec!["--unknown", "relative"], vec!["--startup-config"]] {
        let bytes =
            serde_json::to_vec(&serde_json::json!({"schema_version":1,"arguments":arguments}))?;
        assert!(LaunchDescriptor::parse(&bytes)?.into_startup_paths().is_err());
        assert!(LaunchDescriptor::parse(&bytes)?.maintenance_configuration().is_err());
    }
    Ok(())
}

#[test]
fn preservation_checks_schema_without_rewriting_or_discarding_authored_text() -> Result {
    let bytes =
        "# 保留 café 🙂\r\nschema_version = 1\r\n[custom]\r\ntext = 'ユーザー'\r\n".as_bytes();
    let original = bytes.to_vec();
    initial::validate_preserved(bytes)?;
    assert_eq!(bytes, original);
    assert!(parse_startup_configuration(bytes).is_err());
    for invalid in [
        b"schema_version=2".to_vec(),
        b"schema_version=1\nbad=[".to_vec(),
        b"# no version".to_vec(),
        vec![0xff],
        vec![b' '; MAX_STARTUP_CONFIGURATION_BYTES + 1],
    ] {
        assert!(initial::validate_preserved(&invalid).is_err());
    }
    Ok(())
}

#[test]
fn both_descriptor_consumers_reject_duplicate_and_missing_input_selections() -> Result {
    let original: serde_json::Value = serde_json::from_slice(&LaunchDescriptor::encode(paths()?)?)?;
    for index in [0, 2, 4, 6, 8, 10] {
        let mut duplicate = original.clone();
        duplicate["arguments"][index] = serde_json::json!("--unknown");
        let bytes = serde_json::to_vec(&duplicate)?;
        assert!(LaunchDescriptor::parse(&bytes)?.into_startup_paths().is_err());
        assert!(LaunchDescriptor::parse(&bytes)?.maintenance_configuration().is_err());
        let mut duplicate = original.clone();
        let input = duplicate["arguments"].as_array_mut().ok_or("Arguments")?;
        input.extend_from_within(index..index + 2);
        let bytes = serde_json::to_vec(&duplicate)?;
        assert!(LaunchDescriptor::parse(&bytes)?.into_startup_paths().is_err());
        assert!(LaunchDescriptor::parse(&bytes)?.maintenance_configuration().is_err());
    }
    Ok(())
}

#[test]
fn maintenance_borrows_all_native_inputs_and_preserves_the_first_boundary_error() -> Result {
    let descriptor = LaunchDescriptor::parse(&LaunchDescriptor::encode(paths()?)?)?;
    let mut visited = Vec::new();
    descriptor.validate_maintenance_paths(|path| {
        visited.push(path.to_owned());
        Ok(())
    })?;
    assert_eq!(visited, paths()?.into_paths());
    let mut calls = 0;
    let cause = descriptor
        .validate_maintenance_paths(|_| {
            calls += 1;
            if calls == 3 {
                Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "native boundary"))
            } else {
                Ok(())
            }
        })
        .err()
        .ok_or("Missing original error")?;
    assert_eq!(calls, 3);
    assert_eq!(cause.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(cause.to_string(), "native boundary");
    Ok(())
}

#[cfg(windows)]
#[test]
fn installed_serialization_rejects_nonunicode_without_panicking_or_normalizing() -> Result {
    use std::os::windows::ffi::OsStringExt;
    let raw = PathBuf::from(std::ffi::OsString::from_wide(&[69, 58, 92, 0xdfff]));
    let mut selected = paths()?.into_paths();
    selected[0] = raw.clone();
    let inputs = StartupPaths::new(selected)?;
    assert!(LaunchDescriptor::encode(inputs).is_err());
    assert!(
        initial::managed(&raw, &root().join("python.exe"), &root().join("output"), "en").is_err()
    );
    Ok(())
}
