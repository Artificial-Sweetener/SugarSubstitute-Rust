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
/// Construct only an inert current-process request; roots grant no release trust.
fn make(root: PathBuf) -> Result<CaptureStartInput, CrashError> {
    CaptureStartInput::new(
        root,
        sugarsubstitute_supervision::InstalledReleaseOwner::new().capture_selection(),
    )
}
#[test]
fn absolute_input_preserves_foreign_unicode_and_exact_reservation() -> TestResult {
    let root = path("糖-🧁-root");
    let input = make(root.clone())?;
    assert_eq!(input.capture_root_path(), root);
    assert!(input.selection.is_some());
    assert_eq!(input.required_reservation_bytes(), 4 * 1024 * 1024);
    Ok(())
}
#[test]
fn relative_parent_and_nul_paths_are_rejected_without_io() {
    for root in [PathBuf::from("relative"), path("../root"), path("private\0root")] {
        assert!(make(root).is_err());
    }
}
#[test]
fn original_partitions_and_bundle_have_distinct_fixed_sizes() -> TestResult {
    let input = make(path("root"))?;
    assert_eq!(BASE_STORAGE_BYTES, 2_097_152);
    assert_eq!(input.required_reservation_bytes() - BASE_STORAGE_BYTES, 2_097_152);
    #[cfg(windows)]
    assert_eq!(
        SOURCE_STORAGE_BYTES + NATIVE_STORAGE_BYTES + 65_536 + 1_048_576,
        BASE_STORAGE_BYTES
    );
    Ok(())
}
#[test]
fn short_path_excess_capacity_is_rejected_before_owner_transfer() {
    let mut root = PathBuf::with_capacity(MAX_UNITS * 3 + 1);
    root.push(path("root"));
    assert!(root.as_os_str().len() < 100);
    assert!(make(root).is_err());
}
#[test]
fn path_text_and_ancestor_counts_are_bounded() {
    assert!(make(path(&"x".repeat(MAX_UNITS * 3 + 1))).is_err());
    let mut deep = path("first");
    for _ in 0..MAX_ANCESTORS {
        deep.push("child");
    }
    assert!(make(deep).is_err());
}
#[test]
fn input_debug_redacts_root_and_inert_selection() -> TestResult {
    let input = make(path("private-secret-root"))?;
    let text = format!("{input:?}");
    assert!(!text.contains("private-secret"));
    assert!(!text.contains("CaptureSelectionRequest"));
    Ok(())
}
