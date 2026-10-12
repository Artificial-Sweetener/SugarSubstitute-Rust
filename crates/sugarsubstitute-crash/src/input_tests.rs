// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pure trusted-input bounds are verified independently of native availability.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn path(name: &str) -> PathBuf {
    #[cfg(windows)]
    let base = PathBuf::from(r"E:\capture-bundle");
    #[cfg(not(windows))]
    let base = PathBuf::from("/capture-bundle");
    base.join(name)
}
fn make(client: PathBuf, build: &str) -> Result<CaptureStartInput, CrashError> {
    CaptureStartInput::new(
        client,
        path("handler.exe"),
        path("capture-root"),
        [17; 32],
        [23; 32],
        build,
    )
}
#[test]
fn absolute_input_preserves_foreign_unicode_and_exact_bundle_hashes() -> TestResult {
    let client = path("糖-🧁-client.dll");
    let input = make(client.clone(), "build_17-alpha")?;
    assert_eq!(input.client, client);
    assert_eq!(input.client_hash, [17; 32]);
    assert_eq!(input.handler_hash, [23; 32]);
    assert_eq!(input.required_reservation_bytes(), 2 * 1024 * 1024);
    Ok(())
}
#[test]
fn relative_parent_and_nul_paths_are_rejected_without_io() {
    for client in
        [PathBuf::from("relative.dll"), path("../client.dll"), path("private\0client.dll")]
    {
        assert!(make(client, "build").is_err());
    }
}
#[test]
fn build_identity_has_exact_ascii_and_length_bounds() -> TestResult {
    make(path("client.dll"), &"a".repeat(64))?;
    for build in ["", "private credential", "糖", "a/b", "a.b", &"a".repeat(65)] {
        assert!(make(path("client.dll"), build).is_err());
    }
    Ok(())
}
#[test]
fn short_path_excess_capacity_is_rejected_before_owner_transfer() {
    let mut client = PathBuf::with_capacity(MAX_UNITS * 3 + 1);
    client.push(path("client.dll"));
    assert!(client.as_os_str().len() < 100);
    assert!(make(client, "build").is_err());
}
#[test]
fn path_text_and_ancestor_counts_are_bounded() {
    assert!(make(path(&"x".repeat(MAX_UNITS * 3 + 1)), "build").is_err());
    let mut deep = path("first");
    for _ in 0..MAX_ANCESTORS {
        deep.push("child");
    }
    assert!(make(deep, "build").is_err());
}
#[test]
fn input_debug_redacts_all_paths_build_and_hashes() -> TestResult {
    let input = make(path("private-secret-client.dll"), "secret_build")?;
    let text = format!("{input:?}");
    assert!(!text.contains("private-secret"));
    assert!(!text.contains("secret_build"));
    Ok(())
}
