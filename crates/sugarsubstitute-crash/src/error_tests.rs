// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Redacted diagnostics preserve original typed primary and cleanup causes.
use super::*;
#[test]
fn formatting_redacts_native_primary_and_secondary_sources() {
    let cause =
        CrashError::new(CaptureOperation::Runtime, std::io::Error::other("secret-primary-path"))
            .with_secondary(std::io::Error::other("secret-cleanup-path"));
    assert_eq!(cause.operation(), CaptureOperation::Runtime);
    assert!(!format!("{cause:?} {cause}").contains("secret"));
    assert!(cause.source().is_some_and(|source| source.to_string() == "secret-primary-path"));
    assert_eq!(
        cause.secondary_causes().map(ToString::to_string).collect::<Vec<_>>(),
        ["secret-cleanup-path"]
    );
    let same = cause.clone();
    assert!(cause.same_cause(&same));
}
#[test]
fn admission_formatting_keeps_original_unexpected_execution_cause() {
    use sugarsubstitute_contracts::execution::{ExecutionError, Operation};
    let cause = CrashAdmissionError::Execution(AdmissionError::Unexpected(ExecutionError::new(
        Operation::NativeLifecycle,
        Box::new(std::io::Error::other("secret-token")),
    )));
    assert!(!format!("{cause:?} {cause}").contains("secret-token"));
    assert!(
        cause
            .source()
            .and_then(Error::source)
            .and_then(Error::source)
            .is_some_and(|source| source.to_string() == "secret-token")
    );
}
