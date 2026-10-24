// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve real native installation inputs across a maintenance generation.
use super::*;
use crate::setup_storage::ownership;
use crate::setup_storage::test_worker::{self, TestResult};

use super::fixture::installed as fixture;

#[test]
fn maintenance_preserves_exact_config_and_external_launch_paths() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let original = fixture(&root, context)?;
        let summary = inspect_installation(&root.path, context)?.ok_or("Missing summary")?;
        assert!(!root.path.join(".selection-state").exists());
        assert!(!summary.executable_missing);
        assert_eq!(summary.selection.locale, "ja");
        let existing = ExistingInstallation::read(&root, context)?.ok_or("Missing installation")?;
        let replacement = Directory::open(&root.path.join("releases/replacement"), true)?;
        existing.write_configuration(&replacement.path, context)?;
        assert_eq!(
            std::fs::read(original.join("startup.local.toml"))?,
            std::fs::read(replacement.path.join("startup.local.toml"))?
        );
        let descriptor: LaunchDescriptor = serde_json::from_slice(&std::fs::read(
            replacement.path.join("sugarsubstitute-launch.local.json"),
        )?)?;
        assert_eq!(
            Path::new(&descriptor.arguments[1]),
            replacement.path.join("startup.local.toml")
        );
        assert_eq!(&descriptor.arguments[2..], &existing.descriptor.arguments[2..]);
        assert!(!root.path.join("external-comfy").exists());
        assert!(!root.path.join("models").exists());
        assert!(!root.path.join("outputs").exists());
        Ok(())
    })
}

#[test]
fn missing_executable_is_repairable_without_running_or_reprovisioning_comfy() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let original = fixture(&root, context)?;
        std::fs::remove_file(original.join("sugarsubstitute.exe"))?;
        assert!(
            inspect_installation(&root.path, context)?.ok_or("Missing summary")?.executable_missing
        );
        let existing = ExistingInstallation::read(&root, context)?.ok_or("Missing installation")?;
        let replacement = Directory::open(&root.path.join("releases/repaired"), true)?;
        existing.write_configuration(&replacement.path, context)?;
        files::write_new(&replacement.path.join("sugarsubstitute.exe"), b"replacement fixture")?;
        files::write_new(
            &replacement.path.join("installation-ready.json"),
            &std::fs::read(original.join("installation-ready.json"))?,
        )?;
        entrypoint::stage(&replacement.path)?;
        entrypoint::publish(&root, &replacement.path, context)?;
        assert_eq!(entrypoint::selected(&root)?.ok_or("Missing selection")?.path, replacement.path);
        Ok(())
    })
}

#[test]
fn maintenance_pins_inputs_against_mid_transaction_edit_or_removal() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let original = fixture(&root, context)?;
        let existing = ExistingInstallation::read(&root, context)?.ok_or("Missing installation")?;
        for path in [
            root.path.join(ownership::MARKER),
            original.join("startup.local.toml"),
            original.join("sugarsubstitute-launch.local.json"),
            original.join("installation-ready.json"),
        ] {
            assert!(std::fs::write(&path, b"changed").is_err());
            assert!(std::fs::remove_file(&path).is_err());
        }
        drop(existing);
        std::fs::write(original.join("startup.local.toml"), b"schema_version = 99")?;
        assert!(ExistingInstallation::read(&root, context).is_err());
        Ok(())
    })
}

#[test]
fn damaged_or_redirected_launch_configuration_is_preserved_and_rejected() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let original = fixture(&root, context)?;
        let path = original.join("sugarsubstitute-launch.local.json");
        let mut descriptor: LaunchDescriptor = serde_json::from_slice(&std::fs::read(&path)?)?;
        let external = root.path.join("user.toml");
        std::fs::write(&external, b"schema_version=1")?;
        descriptor.arguments[1] = external.to_string_lossy().into_owned();
        let bytes = serde_json::to_vec(&descriptor)?;
        std::fs::write(&path, &bytes)?;
        assert!(inspect_installation(&root.path, context).is_err());
        assert_eq!(std::fs::read(&path)?, bytes);
        assert_eq!(std::fs::read(&external)?, b"schema_version=1");
        std::fs::write(&path, b"{damaged")?;
        assert!(inspect_installation(&root.path, context).is_err());
        assert_eq!(std::fs::read(&path)?, b"{damaged");
        Ok(())
    })
}

#[test]
fn inspection_does_not_adopt_unknown_populated_directories() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        assert!(inspect_installation(temporary.path(), context)?.is_none());
        std::fs::write(temporary.path().join("user-file"), b"preserve")?;
        assert!(inspect_installation(temporary.path(), context).is_err());
        assert_eq!(std::fs::read(temporary.path().join("user-file"))?, b"preserve");
        assert!(!temporary.path().join(ownership::MARKER).exists());
        Ok(())
    })
}

#[test]
fn missing_shell_entry_recovers_saved_settings_without_turning_into_fresh_setup() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let original = fixture(&root, context)?;
        let saved = std::fs::read(original.join("startup.local.toml"))?;
        std::fs::remove_file(root.path.join("SugarSubstitute.lnk"))?;
        let summary =
            inspect_installation(&root.path, context)?.ok_or("Recovery summary absent")?;
        assert!(summary.launch_entry_missing);
        assert!(!summary.executable_missing);
        assert_eq!(summary.selection.locale, "ja");
        assert_eq!(std::fs::read(original.join("startup.local.toml"))?, saved);
        assert!(!root.path.join("SugarSubstitute.lnk").exists());
        Ok(())
    })
}
