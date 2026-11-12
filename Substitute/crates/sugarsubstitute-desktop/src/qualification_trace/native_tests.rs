// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify original native error chains without changing error custody.
use super::*;
#[test]
fn original_message_has_no_invented_hresult_or_sensitive_text() {
    let error = DesktopError::message("observe_preview_bounds", "private path never transmitted");
    let detail = NativeFailure::from_error(NativePhase::Revalidate, &error);
    assert_eq!(detail.phase, NativePhase::Revalidate);
    assert_eq!(detail.operation, "observe_preview_bounds");
    assert_eq!(detail.hresult, None);
    assert!(error.to_string().contains("private path"));
    let foreign = DesktopError::message("unknown_private_helper", "private detail");
    assert_eq!(NativeFailure::from_error(NativePhase::Seal, &foreign).operation, "Other");
}
#[cfg(windows)]
#[test]
fn callback_and_stream_preserve_original_nested_native_hresult() {
    for (operation, phase, code) in [
        ("complete_main_preview", NativePhase::Callback, 0x80004005u32 as i32),
        ("complete_preview_stream", NativePhase::Stream, 0x80030070u32 as i32),
        ("capture_main_preview", NativePhase::Issue, 0x80004001u32 as i32),
    ] {
        let native = windows::core::Error::from_hresult(windows::core::HRESULT(code));
        let nested = DesktopError::new("external_native_boundary", native);
        let error = DesktopError::new(operation, nested);
        let detail = NativeFailure::from_error(NativePhase::Issue, &error);
        assert_eq!(detail, NativeFailure { phase, operation, hresult: Some(code) });
        assert!(error.source().is_some());
    }
}
#[test]
fn closed_phase_and_operation_reject_arbitrary_external_tokens() {
    assert_eq!(NativePhase::parse("Revalidate"), Some(NativePhase::Revalidate));
    assert_eq!(NativePhase::parse("Secret"), None);
    assert_eq!(native_operation("complete_preview_stream"), Some("complete_preview_stream"));
    assert_eq!(native_operation("private_path"), None);
}
