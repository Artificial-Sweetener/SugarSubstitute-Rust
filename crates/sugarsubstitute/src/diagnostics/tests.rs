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
    let sub = subscriber(health.clone(), move || Fail(sink.clone()), "info".into());
    tracing::subscriber::with_default(sub, || {
        tracing::info!(operation = "diagnostic_fixture", "actual fmt write");
    });
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
