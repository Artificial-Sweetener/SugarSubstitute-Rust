// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify exact semantic identities and lexical bounds without opening native objects.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn canonical_ancestor_retention_never_opens_an_incomplete_verbatim_drive() -> TestResult {
    for prefix in ["E:", r"\\?\E:"] {
        let path = PathBuf::from(format!("{prefix}\\root\\installation"));
        let directories: Vec<_> = ancestor_paths(&path)?.collect();
        assert_eq!(
            directories,
            [PathBuf::from(format!("{prefix}\\")), PathBuf::from(format!("{prefix}\\root"))]
        );
        assert!(directories.iter().all(|path| path.as_os_str() != std::ffi::OsStr::new(prefix)));
    }
    Ok(())
}

fn configuration(
    args: &[&str],
    port: u16,
) -> Result<LaunchConfiguration, Box<dyn std::error::Error>> {
    Ok(LaunchConfiguration::new(
        PathBuf::from("E:\\root"),
        PathBuf::from("E:\\python.exe"),
        args.iter().map(|value| (*value).to_owned()).collect(),
        port,
    )?)
}

#[test]
fn backend_identity_binds_account_actual_objects_argument_boundaries_order_and_port() -> TestResult
{
    let digest =
        backend_hash(&[1], &[2; 24], &[3; 24], &configuration(&["a", "bc", "漢字"], 8188)?);
    assert_ne!(
        digest,
        backend_hash(&[9], &[2; 24], &[3; 24], &configuration(&["a", "bc", "漢字"], 8188)?)
    );
    assert_ne!(
        digest,
        backend_hash(&[1], &[9; 24], &[3; 24], &configuration(&["a", "bc", "漢字"], 8188)?)
    );
    assert_ne!(
        digest,
        backend_hash(&[1], &[2; 24], &[9; 24], &configuration(&["a", "bc", "漢字"], 8188)?)
    );
    assert_ne!(
        digest,
        backend_hash(&[1], &[2; 24], &[3; 24], &configuration(&["ab", "c", "漢字"], 8188)?)
    );
    assert_ne!(
        digest,
        backend_hash(&[1], &[2; 24], &[3; 24], &configuration(&["bc", "a", "漢字"], 8188)?)
    );
    assert_ne!(
        digest,
        backend_hash(&[1], &[2; 24], &[3; 24], &configuration(&["a", "bc", "漢字", ""], 8188)?)
    );
    assert_ne!(
        digest,
        backend_hash(&[1], &[2; 24], &[3; 24], &configuration(&["a", "bc", "漢字"], 8189)?)
    );
    let renamed = LaunchConfiguration::new(
        PathBuf::from("E:\\renamed"),
        PathBuf::from("E:\\renamed\\python.exe"),
        vec!["a".into(), "bc".into(), "漢字".into()],
        8188,
    )?;
    assert_eq!(digest, backend_hash(&[1], &[2; 24], &[3; 24], &renamed));
    Ok(())
}

#[test]
fn native_path_conversion_rejects_remote_device_nul_and_oversized_input() -> TestResult {
    for value in [
        "relative",
        "\\\\server\\share\\path",
        "\\\\?\\UNC\\server\\share\\path",
        "\\\\.\\pipe\\endpoint",
        "E:\\nul\0suffix",
    ] {
        assert!(wide(Path::new(value)).is_err());
    }
    let maximum = format!("E:\\{}", "x".repeat(32763));
    assert_eq!(wide(Path::new(&maximum))?.len(), 32767);
    assert!(wide(Path::new(&format!("{maximum}x"))).is_err());
    assert_eq!(wide(Path::new("E:\\漢字"))?.last(), Some(&0));
    Ok(())
}
