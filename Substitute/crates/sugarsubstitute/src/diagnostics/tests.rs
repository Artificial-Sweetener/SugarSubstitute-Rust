// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the actual subscriber writer and independent first-cause lifetime.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
#[derive(Debug)]
struct Cause(Arc<AtomicUsize>);
impl fmt::Display for Cause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("original sink failure")
    }
}
impl Error for Cause {}
impl Drop for Cause {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}
struct Fail(Arc<AtomicUsize>);
impl Write for Fail {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other(Cause(self.0.clone())))
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(233))
    }
}
#[test]
fn real_subscriber_failed_sink_is_nonrecursive_and_keeps_original_source_until_last_observer()
-> Result<(), Box<dyn Error>> {
    let dropped = Arc::new(AtomicUsize::new(0));
    let health = DiagnosticHealth::fixture();
    let sink = dropped.clone();
    let sub = subscriber(health.clone(), move || Fail(sink.clone()), "info".into(), None);
    tracing::subscriber::with_default(sub, || {
        tracing::info!(operation = "diagnostic_fixture", "actual fmt write");
    });
    // tracing-core interest readers temporarily upgrade other registered subscribers.
    // New registration joins those readers and removes the retired subscriber's weak registrar.
    let _registration_barrier =
        tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let failure = health.check().err().ok_or("Actual sink failure missing")?;
    assert!(
        failure
            .source()
            .and_then(|e| e.downcast_ref::<io::Error>())
            .and_then(|e| e.get_ref())
            .is_some_and(|e| e.is::<Cause>())
    );
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    drop(health);
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    drop(failure);
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn first_secondary_and_omitted_writes_are_bounded_and_original_os_code_is_observable() {
    let health = DiagnosticHealth::fixture();
    let mut writer = Writer { sink: Fail(Arc::new(AtomicUsize::new(0))), health: health.clone() };
    for _ in 0..3 {
        assert!(writer.flush().is_err());
    }
    assert_eq!(health.native_error(), Some(233));
    assert_eq!(health.0.secondary.get().and_then(io::Error::raw_os_error), Some(233));
    assert_eq!(health.0.omitted.load(Ordering::Acquire), 1);
}
#[test]
fn application_failure_and_write_failure_keep_both_independent_sources()
-> Result<(), Box<dyn Error>> {
    let health = DiagnosticHealth::fixture();
    health.retain(io::Error::from_raw_os_error(109));
    let error = resolve::<()>(Err("original application failure".into()), &health)
        .err()
        .ok_or("Actual paired failure missing")?;
    let owned = error.downcast_ref::<LifetimeFailure>().ok_or("Actual paired owner missing")?;
    assert_eq!(
        owned.source().map(ToString::to_string),
        Some("original application failure".into())
    );
    assert_eq!(owned.diagnostic.0.native_error(), Some(109));
    assert!(resolve(Ok(()), &DiagnosticHealth::fixture()).is_ok());
    Ok(())
}

#[test]
fn sensitive_fields_never_invoke_arbitrary_error_formatters() {
    struct Untrusted(Arc<AtomicUsize>);
    impl fmt::Debug for Untrusted {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fetch_add(1, Ordering::AcqRel);
            f.write_str("secret")
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let health = DiagnosticHealth::fixture();
    let subscriber = subscriber(health.clone(), std::io::sink, "trace".into(), None);
    tracing::subscriber::with_default(
        subscriber,
        || tracing::error!(operation="verify_primary_token",error=?Untrusted(calls.clone())),
    );
    assert!(health.check().is_ok());
    assert_eq!(calls.load(Ordering::Acquire), 0);
}

#[test]
fn drained_batches_observe_the_original_shared_stop_cutoff_without_extension() {
    use sugarsubstitute_contracts::execution::Moment;
    let queue = queue::Queue::new();
    let drained = queue.take();
    assert!(drained.frames.is_empty());
    assert_eq!(drained.omitted, 0);
    assert!(!drained.finished);
    queue.stop(Moment::from_nanos(42));
    queue.stop(Moment::from_nanos(999));
    assert_eq!(drained.cutoff.load(Ordering::Acquire), 42);
    assert_eq!(queue.take().cutoff.load(Ordering::Acquire), 42);
}

struct Scripted {
    steps: std::collections::VecDeque<io::Result<usize>>,
    calls: usize,
}
impl Write for Scripted {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        self.steps.pop_front().unwrap_or(Ok(bytes.len()))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn write_all_short_success_is_healthy_but_zero_completion_retains_write_zero()
-> Result<(), Box<dyn Error>> {
    let health = DiagnosticHealth::fixture();
    let mut writer = Writer {
        sink: Scripted { steps: [Ok(1), Ok(2)].into(), calls: 0 },
        health: health.clone(),
    };
    writer.write_all(b"abc")?;
    assert!(health.check().is_ok());
    assert_eq!(writer.sink.calls, 2);
    writer.sink.steps = [Ok(0)].into();
    assert_eq!(
        writer.write_all(b"remaining").err().ok_or("Zero write failure absent")?.kind(),
        io::ErrorKind::WriteZero
    );
    let retained = health.check().err().ok_or("Original write-zero failure absent")?;
    assert_eq!(
        retained.source().and_then(|e| e.downcast_ref::<io::Error>()).map(io::Error::kind),
        Some(io::ErrorKind::WriteZero)
    );
    Ok(())
}
#[test]
fn write_all_interrupted_then_success_does_not_latch_a_terminal_failure()
-> Result<(), Box<dyn Error>> {
    let health = DiagnosticHealth::fixture();
    let mut writer = Writer {
        sink: Scripted {
            steps: [Err(io::Error::from(io::ErrorKind::Interrupted)), Ok(3)].into(),
            calls: 0,
        },
        health: health.clone(),
    };
    writer.write_all(b"abc")?;
    assert_eq!(writer.sink.calls, 2);
    assert!(health.check().is_ok());
    Ok(())
}
#[test]
fn write_all_partial_then_terminal_error_keeps_original_full_source() -> Result<(), Box<dyn Error>>
{
    let dropped = Arc::new(AtomicUsize::new(0));
    let health = DiagnosticHealth::fixture();
    let mut writer = Writer {
        sink: Scripted {
            steps: [Ok(1), Err(io::Error::other(Cause(dropped.clone())))].into(),
            calls: 0,
        },
        health: health.clone(),
    };
    assert!(writer.write_all(b"abc").is_err());
    assert_eq!(writer.sink.calls, 2);
    let failure = health.check().err().ok_or("Terminal source missing")?;
    assert!(
        failure
            .source()
            .and_then(|e| e.downcast_ref::<io::Error>())
            .and_then(io::Error::get_ref)
            .is_some_and(|e| e.is::<Cause>())
    );
    drop(writer);
    drop(health);
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    drop(failure);
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn routed_write_all_preserves_short_and_interrupted_success_without_duplicate_frames()
-> Result<(), Box<dyn Error>> {
    let health = DiagnosticHealth::fixture();
    let mut routed = queue::Routed {
        sink: Writer {
            sink: Scripted {
                steps: [Err(io::ErrorKind::Interrupted.into()), Ok(1), Ok(2)].into(),
                calls: 0,
            },
            health: health.clone(),
        },
        queue: Some(queue::Queue::new()),
        frame: Vec::new(),
        oversized: false,
        written: 0,
        failed: false,
    };
    routed.write_all(b"abc")?;
    assert_eq!(routed.frame, b"abc");
    assert_eq!(routed.sink.sink.calls, 3);
    assert!(!routed.failed);
    assert!(health.check().is_ok());
    routed.sink.sink.steps = [Ok(1)].into();
    assert_eq!(routed.write(b"xy")?, 1);
    assert_eq!(routed.frame, b"abcx");
    Ok(())
}
#[test]
fn complete_event_admission_includes_context_and_omission_without_poisoning_sink_health()
-> Result<(), Box<dyn Error>> {
    let health = DiagnosticHealth::fixture();
    let mut routed = queue::Routed {
        sink: Writer { sink: Vec::new(), health: health.clone() },
        queue: None,
        frame: Vec::new(),
        oversized: false,
        written: 0,
        failed: false,
    };
    for _ in 0..20 {
        if routed.write_all(&vec![b'x'; 1024]).is_err() {
            break;
        }
    }
    assert!(routed.oversized);
    assert!(routed.sink.sink.len() <= queue::EVENT_BYTES);
    assert!(routed.sink.sink.ends_with(b"omitted_events=1\n"));
    assert!(routed.write_all(b"later").is_err());
    assert!(!routed.failed);
    health.check()?;
    Ok(())
}
#[test]
fn routed_partial_terminal_failure_marks_incomplete_data_and_preserves_the_original_cause()
-> Result<(), Box<dyn Error>> {
    let dropped = Arc::new(AtomicUsize::new(0));
    let health = DiagnosticHealth::fixture();
    let mut routed = queue::Routed {
        sink: Writer {
            sink: Scripted {
                steps: [Ok(1), Err(io::Error::other(Cause(dropped.clone())))].into(),
                calls: 0,
            },
            health: health.clone(),
        },
        queue: Some(queue::Queue::new()),
        frame: Vec::new(),
        oversized: false,
        written: 0,
        failed: false,
    };
    assert!(routed.write_all(b"abc").is_err());
    assert!(routed.failed);
    assert!(routed.frame.is_empty());
    let failure = health.check().err().ok_or("Original routed failure absent")?;
    assert!(
        failure
            .source()
            .and_then(|cause| cause.downcast_ref::<io::Error>())
            .and_then(io::Error::get_ref)
            .is_some_and(|cause| cause.is::<Cause>())
    );
    drop(routed);
    drop(health);
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    drop(failure);
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    Ok(())
}
