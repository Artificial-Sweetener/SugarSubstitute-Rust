// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify native canonical intake without admitting remote, device or ambiguous filesystem names.
use super::*;

#[test]
fn canonical_local_drive_paths_preserve_the_same_strict_component_policy() {
    for valid in [r"C:\owned\directory", r"\\?\C:\owned\directory"] {
        assert!(lexical(Path::new(valid)).is_ok(), "rejected {valid}");
    }
    for invalid_path in [
        r"C:\",
        r"\\?\C:\",
        r"C:relative",
        r"\\server\share\directory",
        r"\\?\UNC\server\share\directory",
        r"\\.\C:\directory",
        r"\\?\GLOBALROOT\Device\HarddiskVolume1\directory",
        r"\\?\C:\owned\..\other",
        r"\\?\C:\owned\.\other",
        r"\\?\C:\owned\NUL",
        r"\\?\C:\owned\trailing.",
        r"\\?\C:\owned\record:stream",
    ] {
        assert!(lexical(Path::new(invalid_path)).is_err(), "accepted {invalid_path}");
    }
}

#[test]
fn canonical_native_directory_keeps_identity_and_real_pins_through_retention_and_resolution()
-> crate::setup_storage::test_worker::TestResult {
    crate::setup_storage::test_worker::run_reserved(8 * 1024 * 1024, |_, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("owned");
        std::fs::create_dir(&path)?;
        let identity = Directory::open(&path, false)?.identity()?;
        let native_path = std::fs::canonicalize(&path)?;
        let directory = Directory::open(&native_path, false)?;
        assert_eq!(directory.path(), native_path);
        assert_eq!(directory.identity()?, identity);
        let retained = directory.retained()?;
        let resolved = directory.resolved()?;
        assert_eq!(resolved.identity()?, identity);
        let moved = temporary.path().join("retired");
        assert!(std::fs::rename(&path, &moved).is_err());
        drop(resolved);
        assert!(std::fs::rename(&path, &moved).is_err());
        drop(retained);
        std::fs::rename(&path, &moved)?;
        Ok(())
    })
}
