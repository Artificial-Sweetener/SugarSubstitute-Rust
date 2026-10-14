// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual retained-error ownership using a private structured writer boundary.
use super::*;
use crate::DesktopError;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tracing::field::{Field, Visit};
use tracing::{
    Event, Metadata, Subscriber,
    span::{Attributes, Id, Record},
};

#[test]
fn closed_first_failure_maps_only_known_boundary_and_direct_error_kinds() {
    let io = std::io::Error::other("private payload");
    for (operation, expected) in [
        ("poll_frame", Boundary::PollFrame),
        ("request_frame", Boundary::RequestFrame),
        ("observe_startup", Boundary::ObserveStartup),
        ("take_startup_command", Boundary::TakeCommand),
        ("set_content_visibility", Boundary::Visibility),
        ("startup_preparation", Boundary::PreSurface),
        ("hide_content", Boundary::Cleanup),
        ("request_shutdown", Boundary::Cleanup),
        ("poll_shutdown", Boundary::Cleanup),
        ("finish_preparation_shutdown", Boundary::Cleanup),
        ("finish_startup_shutdown", Boundary::Cleanup),
        ("present_startup_splash", Boundary::Presentation),
        ("query_cursor_monitor", Boundary::CursorMonitor),
        ("query_monitor_work_area", Boundary::MonitorWorkArea),
        ("center_window", Boundary::CenterWindow),
        ("credential private unknown", Boundary::Other),
    ] {
        assert_eq!(classify(operation, &io).boundary, expected);
    }
    for (kind, expected) in [
        (ErrorKind::TimedOut, Kind::TimedOut),
        (ErrorKind::BrokenPipe, Kind::Disconnected),
        (ErrorKind::ConnectionAborted, Kind::Disconnected),
        (ErrorKind::ConnectionReset, Kind::Disconnected),
        (ErrorKind::NotConnected, Kind::Disconnected),
        (ErrorKind::PermissionDenied, Kind::Denied),
        (ErrorKind::InvalidData, Kind::InvalidInput),
        (ErrorKind::InvalidInput, Kind::InvalidInput),
        (ErrorKind::Other, Kind::Io),
    ] {
        assert_eq!(classify("poll_frame", &std::io::Error::from(kind)).kind, expected);
    }
}

struct Source {
    formatted: Arc<AtomicUsize>,
    inspected: Arc<AtomicUsize>,
    dropped: Arc<AtomicUsize>,
}
impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Source")
    }
}
impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.formatted.fetch_add(1, Ordering::SeqCst);
        f.write_str("credential PRIVATE")
    }
}
impl Error for Source {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.inspected.fetch_add(1, Ordering::SeqCst);
        None
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

type RecordedEvent = Vec<(String, String)>;
#[derive(Clone)]
struct Sink(Arc<Mutex<Vec<RecordedEvent>>>);
struct Fields(RecordedEvent);
impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push((field.name().to_owned(), format!("{value:?}")));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
    }
}
impl Subscriber for Sink {
    fn enabled(&self, meta: &Metadata<'_>) -> bool {
        meta.target().ends_with("contract::failure_diagnostic")
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields(Vec::new());
        event.record(&mut fields);
        if let Ok(mut events) = self.0.lock() {
            events.push(fields.0);
        }
    }
}
#[test]
fn actual_first_cause_is_owned_before_one_closed_event_without_formatting_or_source_walk()
-> Result<(), Box<dyn Error>> {
    let formatted = Arc::new(AtomicUsize::new(0));
    let inspected = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut failure = None;
    let source = Source {
        formatted: formatted.clone(),
        inspected: inspected.clone(),
        dropped: dropped.clone(),
    };
    tracing::dispatcher::with_default(&tracing::Dispatch::new(Sink(events.clone())), || {
        DesktopError::retain_first(&mut failure, DesktopError::new("poll_frame", source));
        DesktopError::retain_first(
            &mut failure,
            DesktopError::new("finish_startup_shutdown", std::io::Error::other("PRIVATE cleanup")),
        );
    });
    let actual = failure.as_ref().ok_or("Original cause lost")?;
    assert!(actual.source().and_then(|cause| cause.downcast_ref::<Source>()).is_some());
    assert_eq!(formatted.load(Ordering::SeqCst), 0);
    assert_eq!(inspected.load(Ordering::SeqCst), 0);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    let events = events.lock().map_err(|_| std::io::Error::other("Sink poisoned"))?;
    assert_eq!(events.len(), 1);
    assert!(events[0].iter().any(|(k, v)| k == "operation" && v == "desktop_failure_transition"));
    assert!(events[0].iter().any(|(k, v)| k == "boundary" && v == "PollFrame"));
    assert!(events[0].iter().any(|(k, v)| k == "kind" && v == "Other"));
    assert!(!events[0].iter().any(|(_, v)| v.contains("PRIVATE") || v.contains("credential")));
    drop(failure);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn main_failure_codes_preserve_direct_native_and_cdp_classifications() -> Result<(), Box<dyn Error>>
{
    use crate::main_readiness_types::ReadinessFault;
    assert_eq!(
        main_code(&ReadinessFault::WrongOrigin),
        MainCode::Readiness(ReadinessFault::WrongOrigin)
    );
    assert_eq!(main_code(&std::io::Error::from_raw_os_error(233)), MainCode::Os(233));
    assert_eq!(main_code(&std::io::Error::from(ErrorKind::TimedOut)), MainCode::Io(Kind::TimedOut));
    let invalid =
        serde_json::from_str::<u32>("\"PRIVATE\"").err().ok_or("Missing actual JSON error")?;
    assert_eq!(main_code(&invalid), MainCode::JsonData);
    #[cfg(target_os = "windows")]
    assert_eq!(
        main_code(&windows::core::Error::from(windows::Win32::Foundation::E_POINTER)),
        MainCode::Hresult(windows::Win32::Foundation::E_POINTER.0)
    );
    Ok(())
}

#[test]
fn main_failure_observer_preserves_opaque_cause_through_actual_startup_transfer()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::{
        execution::Generation,
        lifecycle::{
            CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
        },
        startup::{LaunchId, StartupFailure, StartupFailureCode},
    };
    let formatted = Arc::new(AtomicUsize::new(0));
    let inspected = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let events = Arc::new(Mutex::new(Vec::new()));
    let actual = DesktopError::new(
        "decode_main_observation",
        Source {
            formatted: formatted.clone(),
            inspected: inspected.clone(),
            dropped: dropped.clone(),
        },
    );
    let identity = crate::MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(3)?,
        navigation: Generation::new(4)?,
    };
    tracing::dispatcher::with_default(&tracing::Dispatch::new(Sink(events.clone())), || {
        actual.observe_main_failure(identity, "Frame");
    });
    let failure = StartupFailure::new(StartupFailureCode::Presentation, Box::new(actual));
    assert_eq!(formatted.load(Ordering::SeqCst), 0);
    assert_eq!(inspected.load(Ordering::SeqCst), 0);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    let events = events.lock().map_err(|_| std::io::Error::other("Sink poisoned"))?;
    assert_eq!(events.len(), 1);
    assert!(events[0].iter().any(|(k, v)| k == "operation" && v == "main_failure_transition"));
    assert!(
        events[0].iter().any(|(k, v)| k == "failed_operation" && v == "decode_main_observation")
    );
    assert!(events[0].iter().any(|(k, v)| k == "code" && v == "Opaque"));
    assert!(!events[0].iter().any(|(_, v)| v.contains("PRIVATE") || v.contains("credential")));
    drop(failure);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    Ok(())
}
