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
#[test]
fn actual_startup_observation_enriches_one_generic_frame_and_preserves_opaque_cause()
-> Result<(), Box<dyn Error>> {
    use crate::{
        DesktopStartupObservation, StartupCaptureState, StartupCauseCode, StartupCloseFailure,
        StartupCloseStage, StartupNativeCause,
    };
    let formatted = Arc::new(AtomicUsize::new(0));
    let inspected = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let source = Source {
        formatted: formatted.clone(),
        inspected: inspected.clone(),
        dropped: dropped.clone(),
    };
    let cause = StartupCloseFailure::retain(
        StartupCloseStage::CaptureObservation,
        StartupCaptureState::CounterFailed,
        StartupCauseCode::Native(StartupNativeCause::CounterConversion, None),
        Box::new(source),
    );
    let kind = DesktopError::startup_observation_kind(&DesktopStartupObservation::CloseRequested);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut first = None;
    tracing::dispatcher::with_default(&tracing::Dispatch::new(Sink(events.clone())), || {
        DesktopError::retain_first(&mut first, DesktopError::observed_startup(kind, cause));
    });
    let fields = events.lock().map_err(|_| std::io::Error::other("Sink poisoned"))?;
    assert_eq!(fields.len(), 1);
    for (name, value) in [
        ("operation", "desktop_failure_transition"),
        ("boundary", "ObserveStartup"),
        ("kind", "Other"),
        ("observation", "CloseRequested"),
        ("close_stage", "CaptureObservation"),
        ("capture_snapshot", "CounterFailed"),
        ("code", "Native(CounterConversion)"),
    ] {
        assert!(fields[0].iter().any(|(key, actual)| key == name && actual == value));
    }
    assert!(
        !fields[0]
            .iter()
            .any(|(_, value)| value.contains("PRIVATE") || value.contains("credential"))
    );
    assert_eq!(formatted.load(Ordering::SeqCst), 0);
    assert_eq!(inspected.load(Ordering::SeqCst), 0);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    let actual = first.as_ref().ok_or("Original cause absent")?;
    let close = actual
        .source()
        .and_then(Error::source)
        .and_then(|cause| cause.downcast_ref::<StartupCloseFailure>())
        .ok_or("Typed close facts lost")?;
    assert!(close.original().downcast_ref::<Source>().is_some());
    assert_eq!(inspected.load(Ordering::SeqCst), 0);
    drop(first);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    Ok(())
}
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
            || meta.target().ends_with("contract::failure_diagnostic::startup")
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

#[test]
fn actual_poll_frame_consumer_failure_emits_one_closed_stage_without_inferred_observation_or_cause_callbacks()
-> Result<(), Box<dyn Error>> {
    let formatted = Arc::new(AtomicUsize::new(0));
    let inspected = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let events = Arc::new(Mutex::new(Vec::new()));
    let original = Box::new(Source {
        formatted: formatted.clone(),
        inspected: inspected.clone(),
        dropped: dropped.clone(),
    });
    let address = original.as_ref() as *const Source;
    let cause = crate::StartupConsumeFailure::retain(
        crate::StartupConsumeStage::ChildDelivery,
        crate::StartupCauseCode::Bootstrap(crate::StartupBootstrapCause::Transport),
        original,
    );
    let mut first = None;
    tracing::dispatcher::with_default(&tracing::Dispatch::new(Sink(events.clone())), || {
        DesktopError::retain_first(&mut first, DesktopError::boxed("poll_frame", cause));
        DesktopError::retain_first(
            &mut first,
            DesktopError::new("observe_startup", std::io::Error::other("PRIVATE")),
        );
    });
    let actual = first
        .as_ref()
        .ok_or("First cause absent")?
        .cause
        .downcast_ref::<crate::StartupConsumeFailure>()
        .ok_or("Consumer cause absent")?;
    assert!(std::ptr::eq(
        address,
        actual.original().downcast_ref::<Source>().ok_or("Original source absent")?
    ));
    let events = events.lock().map_err(|_| std::io::Error::other("Fixture lock"))?;
    assert_eq!(events.len(), 1);
    for (key, value) in [
        ("operation", "desktop_failure_transition"),
        ("boundary", "PollFrame"),
        ("kind", "Other"),
        ("consume_stage", "ChildDelivery"),
        ("category", "Bootstrap(Transport)"),
    ] {
        assert!(events[0].iter().any(|(k, v)| k == key && v == value));
    }
    assert!(!events[0].iter().any(|(k, v)| k == "observation"
        || k == "close_stage"
        || k == "capture_snapshot"
        || v.contains("PRIVATE")));
    assert_eq!(formatted.load(Ordering::SeqCst), 0);
    assert_eq!(inspected.load(Ordering::SeqCst), 0);
    drop(events);
    drop(first);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn actual_main_absence_observation_tag_remains_distinct_from_retired_without_identity_projection()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::{
        execution::Generation,
        lifecycle::{
            CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
        },
        startup::LaunchId,
    };
    let identity = crate::MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    };
    assert_eq!(
        DesktopError::startup_observation_kind(&crate::DesktopStartupObservation::MainAbsent(
            identity
        )),
        "MainAbsent"
    );
    assert_eq!(
        DesktopError::startup_observation_kind(&crate::DesktopStartupObservation::MainRetired(
            identity
        )),
        "MainRetired"
    );
    Ok(())
}

#[test]
fn frontend_timeout_actual_formatter_emits_only_bounded_sealed_owner_fields()
-> Result<(), Box<dyn Error>> {
    use crate::main_readiness::snapshot::{Snapshot, Wait};
    use sugarsubstitute_contracts::{
        execution::Generation,
        lifecycle::{
            CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
        },
        startup::LaunchId,
    };
    let identity = crate::MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(u64::MAX)?,
        navigation: Generation::new(u64::MAX)?,
    };
    let events = Arc::new(Mutex::new(Vec::new()));
    tracing::dispatcher::with_default(&tracing::Dispatch::new(Sink(events.clone())), || {
        crate::DesktopError::observe_frontend_timeout(
            identity,
            Snapshot { wait: Wait::Barrier, operation: Some(u64::MAX) },
        );
    });
    let captured = events.lock().map_err(|_| std::io::Error::other("Sink poisoned"))?;
    assert_eq!(captured.len(), 1);
    for (key, value) in [
        ("operation", "main_failure_transition"),
        ("failed_operation", "frontend_budget_expired"),
        ("observation", "None"),
        ("code", "Io(TimedOut)"),
        ("wait", "Barrier"),
        ("probe_operation", "18446744073709551615"),
    ] {
        assert!(captured[0].iter().any(|(k, v)| k == key && v == value));
    }
    let bytes: usize = captured[0].iter().map(|(k, v)| k.len() + v.len() + 4).sum();
    assert!(bytes + 128 < 512);
    Ok(())
}
