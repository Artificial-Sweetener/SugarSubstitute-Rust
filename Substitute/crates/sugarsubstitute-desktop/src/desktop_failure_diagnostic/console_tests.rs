// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify actual first-failure console projection without invoking opaque causes.
use super::*;
use crate::console_failure_diagnostic::ConsoleFailureCode as Code;
use crate::console_failure_diagnostic::{
    ConsoleFailureIoKind as IoKind, ConsoleFailureOperation as Operation,
};

#[test]
fn console_direct_codes_accept_supported_types_and_numeric_extremes() -> Result<(), Box<dyn Error>>
{
    for value in [i32::MIN, -1, 0, 1447, i32::MAX] {
        assert_eq!(Code::direct(&std::io::Error::from_raw_os_error(value)), Code::Os(value));
    }
    #[cfg(target_os = "windows")]
    for value in [i32::MIN, -1] {
        assert_eq!(
            Code::direct(&windows::core::Error::from_hresult(windows::core::HRESULT(value))),
            Code::Hresult(value)
        );
    }
    for (kind, expected) in [
        (ErrorKind::TimedOut, IoKind::TimedOut),
        (ErrorKind::BrokenPipe, IoKind::Disconnected),
        (ErrorKind::PermissionDenied, IoKind::Denied),
        (ErrorKind::InvalidInput, IoKind::InvalidInput),
        (ErrorKind::Other, IoKind::Io),
    ] {
        assert_eq!(Code::direct(&std::io::Error::from(kind)), Code::Io(expected));
    }
    let integer = u8::try_from(256_u16).err().ok_or("Integer rejection absent")?;
    assert_eq!(Code::direct(&integer), Code::IntegerRange);
    let allocation =
        Vec::<u8>::new().try_reserve(usize::MAX).err().ok_or("Allocation rejection absent")?;
    assert_eq!(Code::direct(&allocation), Code::Allocation);
    let claim = sugarsubstitute_contracts::execution::MemoryClaimError {
        requested_bytes: u64::MAX,
        available_bytes: 0,
    };
    assert_eq!(Code::direct(&claim), Code::MemoryClaim);
    Ok(())
}

#[test]
fn console_operation_requires_an_exact_authored_stage() {
    assert_eq!(Operation::from_operation("console_scroll_observe"), Some(Operation::ScrollObserve));
    assert_eq!(
        Operation::from_operation("console_accessibility_create"),
        Some(Operation::AccessibilityCreate)
    );
    assert_eq!(Operation::from_operation("console_claim"), Some(Operation::Claim));
    for operation in [
        "console_unknown",
        "console_scroll_observe\nPRIVATE",
        "console_scroll_observe ",
        "console_fixture_window",
        "console_unresolved_drop",
        "CONSOLE_scroll_observe",
        "console_scroll_observe/PRIVATE",
        "poll_frame",
    ] {
        assert_eq!(Operation::from_operation(operation), None);
    }
}

#[test]
fn console_first_failure_keeps_exact_direct_box_and_opaque_callbacks_untouched()
-> Result<(), Box<dyn Error>> {
    let formatted = Arc::new(AtomicUsize::new(0));
    let inspected = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let original = Box::new(Source {
        formatted: formatted.clone(),
        inspected: inspected.clone(),
        dropped: dropped.clone(),
    });
    let address = original.as_ref() as *const Source;
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut first = None;
    tracing::dispatcher::with_default(&tracing::Dispatch::new(Sink(events.clone())), || {
        DesktopError::retain_first(
            &mut first,
            DesktopError::boxed("console_scroll_observe", original),
        );
        DesktopError::retain_first(
            &mut first,
            DesktopError::new("console_window_destroy", std::io::Error::from_raw_os_error(5)),
        );
    });
    let retained = first
        .as_ref()
        .ok_or("First cause absent")?
        .cause
        .downcast_ref::<Source>()
        .ok_or("Original direct source absent")?;
    assert!(std::ptr::eq(address, retained));
    assert_eq!(formatted.load(Ordering::SeqCst), 0);
    assert_eq!(inspected.load(Ordering::SeqCst), 0);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    let captured = events.lock().map_err(|_| std::io::Error::other("Sink poisoned"))?;
    assert_eq!(captured.len(), 1);
    for (key, value) in [
        ("operation", "desktop_failure_transition"),
        ("boundary", "Other"),
        ("kind", "Other"),
        ("console_operation", "ScrollObserve"),
        ("console_code", "Opaque"),
    ] {
        assert!(captured[0].iter().any(|(k, v)| k == key && v == value));
    }
    assert!(
        !captured[0]
            .iter()
            .any(|(_, value)| value.contains("PRIVATE") || value.contains("credential"))
    );
    drop(captured);
    drop(first);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn console_wrapped_io_cause_projects_only_direct_kind_without_walking_nested_source()
-> Result<(), Box<dyn Error>> {
    let formatted = Arc::new(AtomicUsize::new(0));
    let inspected = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let cause = std::io::Error::other(Source {
        formatted: formatted.clone(),
        inspected: inspected.clone(),
        dropped: dropped.clone(),
    });
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut first = None;
    tracing::dispatcher::with_default(&tracing::Dispatch::new(Sink(events.clone())), || {
        DesktopError::retain_first(&mut first, DesktopError::new("console_text_stream", cause));
    });
    assert_eq!(formatted.load(Ordering::SeqCst), 0);
    assert_eq!(inspected.load(Ordering::SeqCst), 0);
    let captured = events.lock().map_err(|_| std::io::Error::other("Sink poisoned"))?;
    assert_eq!(captured.len(), 1);
    assert!(captured[0].iter().any(|(k, v)| k == "console_code" && v == "Io(Io)"));
    assert!(
        first
            .as_ref()
            .ok_or("First cause absent")?
            .cause
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            .and_then(|source| source.downcast_ref::<Source>())
            .is_some()
    );
    drop(captured);
    drop(first);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn console_native_scroll_failure_projects_actual_operation_and_direct_code()
-> Result<(), Box<dyn Error>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut first = None;
    #[cfg(target_os = "windows")]
    let cause = windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(1447));
    #[cfg(not(target_os = "windows"))]
    let cause = std::io::Error::from_raw_os_error(1447);
    tracing::dispatcher::with_default(&tracing::Dispatch::new(Sink(events.clone())), || {
        DesktopError::retain_first(&mut first, DesktopError::new("console_scroll_observe", cause));
    });
    let captured = events.lock().map_err(|_| std::io::Error::other("Sink poisoned"))?;
    assert_eq!(captured.len(), 1);
    assert!(captured[0].iter().any(|(k, v)| k == "console_operation" && v == "ScrollObserve"));
    #[cfg(target_os = "windows")]
    let expected = "Hresult(-2147023449)";
    #[cfg(not(target_os = "windows"))]
    let expected = "Os(1447)";
    assert!(captured[0].iter().any(|(k, v)| k == "console_code" && v == expected));
    Ok(())
}

#[test]
fn unknown_console_operations_and_legacy_failures_keep_original_event_fields()
-> Result<(), Box<dyn Error>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    tracing::dispatcher::with_default(&tracing::Dispatch::new(Sink(events.clone())), || {
        for operation in ["console_PRIVATE_unknown", "poll_frame"] {
            let mut first = None;
            DesktopError::retain_first(
                &mut first,
                DesktopError::new(operation, std::io::Error::from(ErrorKind::TimedOut)),
            );
        }
    });
    let captured = events.lock().map_err(|_| std::io::Error::other("Sink poisoned"))?;
    assert_eq!(captured.len(), 2);
    for (event, boundary) in captured.iter().zip(["Other", "PollFrame"]) {
        assert_eq!(
            event,
            &vec![
                ("message".into(), "First owned desktop failure retained".into()),
                ("operation".into(), "desktop_failure_transition".into()),
                ("boundary".into(), boundary.into()),
                ("kind".into(), "TimedOut".into()),
            ]
        );
    }
    Ok(())
}
