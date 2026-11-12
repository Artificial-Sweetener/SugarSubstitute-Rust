// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Probe entry parses bounded local correlation without accepting feed or trust overrides.
use super::*;

/// Serialize inert public correlation without opening an installation.
fn encoded() -> String {
    serde_json::json!({"installation":"1".repeat(32),"generation":{"id":"2".repeat(32),"package":"3".repeat(64),"release":{"version":"1.0.0","sequence":1}},"revision":7}).to_string()
}

#[test]
fn probe_arguments_preserve_unicode_paths_and_exact_attempts() -> io::Result<()> {
    let root = if cfg!(windows) { r"E:\unopened\工程 Sugar" } else { "/unopened/工程 Sugar" };
    let (installation, request, handle) = parse([
        OsString::from(PROBE_ARGUMENT),
        OsString::from(root),
        encoded().into(),
        "128".into(),
    ])?
    .ok_or_else(|| io::Error::other("Probe absent"))?;
    assert_eq!(handle, 128);
    assert_eq!(installation, PathBuf::from(root));
    assert_eq!(request.attempt().revision, 7);
    Ok(())
}

#[test]
fn incomplete_relative_oversized_and_authority_override_probe_arguments_fail() {
    let root = if cfg!(windows) { r"E:\app" } else { "/app" };
    for values in [
        vec![],
        vec![root.to_owned()],
        vec!["relative".to_owned(), encoded()],
        vec![root.to_owned(), " ".repeat(MAX_PROBE_BYTES + 1)],
        vec![root.to_owned(), encoded(), "https://foreign.test/".to_owned()],
        vec![root.to_owned(), encoded(), "128".to_owned(), "extra".to_owned()],
        vec![root.to_owned(), encoded(), "0".to_owned()],
        vec![root.to_owned(), encoded(), "+128".to_owned()],
        vec![root.to_owned(), encoded(), "0128".to_owned()],
        vec![root.to_owned(), encoded(), usize::MAX.to_string()],
        vec![root.to_owned(), "{}".to_owned()],
    ] {
        assert!(
            parse(
                std::iter::once(PROBE_ARGUMENT.into())
                    .chain(values.into_iter().map(OsString::from))
            )
            .is_err()
        );
    }
}

#[test]
fn unsupported_probe_versions_cannot_fall_back_to_an_ordinary_entry() -> io::Result<()> {
    for version in ["--internal-updater-probe-v1", "--internal-updater-probe-v3"] {
        assert!(parse([version.into()]).is_err());
    }
    for input in [vec![], vec!["--internal-updater-prepare-v1"], vec!["--setup"]] {
        assert!(parse(input.into_iter().map(OsString::from))?.is_none());
    }
    Ok(())
}
