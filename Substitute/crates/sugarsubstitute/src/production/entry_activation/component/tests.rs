// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject incomplete qualification grammar before entry election, capture or native effects.
use super::*;
use std::ffi::OsString;
#[test]
fn component_requires_actual_production_entry_grammar_before_effects() {
    for arguments in [
        vec![],
        vec!["--internal-native-exit-fixture", "0"],
        vec!["--production"],
        vec!["--production", "--installation-root", "relative"],
    ] {
        assert!(ProfileComponent::command(arguments.into_iter().map(OsString::from)).is_err());
    }
}
#[test]
fn component_retains_explicit_installed_and_profile_paths_for_authentic_entry() -> Result<()> {
    let command = ProfileComponent::command(
        [
            "--production",
            "--installation-root",
            r"D:\owned-component\installed",
            "--startup-config",
            r"D:\owned-component\startup.toml",
            "--frontend-package",
            r"D:\owned-component\frontend",
            "--frontend-metadata",
            r"D:\owned-component\metadata.json",
            "--sugarcubes-root",
            r"D:\owned-component\cubes",
            "--backend-script",
            r"D:\owned-component\main.py",
            "--profile-directory",
            r"D:\owned-component\profile",
        ]
        .map(OsString::from),
    )?;
    assert_eq!(command.installation, PathBuf::from(r"D:\owned-component\installed"));
    let sugarsubstitute_configuration::paths::ConfigurationSource::Explicit(paths) =
        command.options
    else {
        return Err("Explicit launch paths missing".into());
    };
    assert_eq!(paths.configuration(), PathBuf::from(r"D:\owned-component\startup.toml"));
    Ok(())
}

#[test]
fn component_reply_probe_preserves_setup_and_caps_before_addition() -> Result<()> {
    let now = Moment::from_nanos(10_000_000_000);
    let setup = Moment::from_nanos(30_000_000_000);
    let probe = ProfileComponent::reply_probe(now, setup)?;
    assert_eq!(probe, Moment::from_nanos(12_000_000_000));
    assert_eq!(setup, Moment::from_nanos(30_000_000_000));
    assert_eq!(
        ProfileComponent::reply_probe(
            Moment::from_nanos(u64::MAX - 1),
            Moment::from_nanos(u64::MAX)
        )?,
        Moment::from_nanos(u64::MAX)
    );
    assert!(ProfileComponent::reply_probe(setup, now).is_err());
    Ok(())
}
