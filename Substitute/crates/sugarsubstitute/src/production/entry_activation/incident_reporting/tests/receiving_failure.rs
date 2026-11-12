// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual receiving error owners survive bounded secondary diagnostics and repeated cleanup errors.
use super::*;
use std::{
    fmt,
    io::Write,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
#[derive(Debug)]
struct Cause {
    code: u32,
    dropped: Arc<AtomicUsize>,
}
impl fmt::Display for Cause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Receiving cause {}", self.code)
    }
}
impl Error for Cause {}
impl Drop for Cause {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::AcqRel);
    }
}
fn cause(code: u32, dropped: &Arc<AtomicUsize>) -> Box<dyn Error> {
    Box::new(Cause { code, dropped: dropped.clone() })
}
fn pointer(cause: &(dyn Error + 'static)) -> *const () {
    std::ptr::from_ref(cause).cast::<()>()
}
#[derive(Clone)]
struct Sink(Arc<Mutex<Vec<u8>>>);
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Sink {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}
impl Write for Sink {
    fn write(&mut self, value: &[u8]) -> std::io::Result<usize> {
        let mut bytes =
            self.0.lock().map_err(|_| std::io::Error::other("Trace fixture poisoned"))?;
        if bytes.len().saturating_add(value.len()) > 8192 {
            return Err(std::io::Error::other("Trace fixture capacity"));
        }
        bytes.extend_from_slice(value);
        Ok(value.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn recorded(emit: impl FnOnce() -> Result<()>) -> Result<String> {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    // Keep shared first-registration interest independent of unsubscribed test threads.
    // The pinned tracing-core fast path otherwise samples their empty default.
    let _callsite_registry = tracing::Dispatch::new(tracing_subscriber::registry());
    let subscriber = tracing_subscriber::fmt()
        .with_writer(Sink(bytes.clone()))
        .without_time()
        .with_ansi(false)
        .finish();
    tracing::subscriber::with_default(subscriber, emit)?;
    let bytes = bytes.lock().map_err(|_| "Trace fixture poisoned")?.clone();
    Ok(String::from_utf8(bytes)?)
}
/// Exercise one shared callsite through the actual fixture formatter and an empty dispatcher.
fn capture_probe(marker: &str) {
    tracing::error!(
        operation = "reporter_receiving_capture_fixture",
        marker,
        "Scoped receiving capture probe"
    );
}
#[test]
fn recorded_capture_survives_unsubscribed_first_registration_of_its_actual_callsite() -> Result<()>
{
    let trace = recorded(|| {
        tracing::dispatcher::with_default(&tracing::Dispatch::none(), || {
            capture_probe("unsubscribed-first");
        });
        capture_probe("captured");
        Ok(())
    })?;
    assert_eq!(trace.lines().count(), 1);
    assert_eq!(trace.matches("operation=\"reporter_receiving_capture_fixture\"").count(), 1);
    assert!(trace.contains("captured"));
    assert!(!trace.contains("unsubscribed-first"));
    Ok(())
}
#[test]
fn reporter_secondary_transition_retains_exact_error_and_original_first_identity() -> Result<()> {
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut receiving = IncidentReporting::new();
    let first = cause(1, &dropped);
    let first_pointer = pointer(first.as_ref());
    assert!(receiving.retain_transition_failure(ReportTransition::CaptureOutput, first).is_err());
    let second = cause(2, &dropped);
    let second_pointer = pointer(second.as_ref());
    receiving.retain_transition_failure(ReportTransition::Retire, second)?;
    assert_eq!(pointer(receiving.failure().ok_or("First cause missing")?), first_pointer);
    let secondary = receiving.secondary_receiving_failure.as_ref().ok_or("Second cause missing")?;
    assert_eq!(pointer(secondary.cause.as_ref()), second_pointer);
    assert!(matches!(secondary.transition, ReportTransition::Retire));
    assert_eq!(secondary.cause.downcast_ref::<Cause>().map(|cause| cause.code), Some(2));
    assert_eq!(receiving.omitted_receiving_failures, 0);
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    drop(receiving);
    assert_eq!(dropped.load(Ordering::Acquire), 2);
    Ok(())
}
#[test]
fn reporter_repeated_secondary_failures_count_omissions_with_three_finite_diagnostics() -> Result<()>
{
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut receiving = IncidentReporting::new();
    let original = IncidentOperationBudget::new(Moment::from_nanos(17))?;
    receiving.report_budget = Some(original);
    receiving.queue_until = Some(original.until());
    let trace = recorded(|| {
        assert!(
            receiving
                .retain_transition_failure(ReportTransition::CaptureOutput, cause(1, &dropped))
                .is_err()
        );
        receiving.retain_transition_failure(ReportTransition::Retire, cause(2, &dropped))?;
        for code in 3..1003 {
            receiving.retain_transition_failure(ReportTransition::Retire, cause(code, &dropped))?;
        }
        Ok(())
    })?;
    assert_eq!(trace.lines().count(), 3);
    for operation in [
        "reporter_transition_failed",
        "reporter_secondary_transition_failed",
        "reporter_transition_failures_omitted",
    ] {
        assert_eq!(trace.matches(&format!("operation=\"{operation}\"")).count(), 1);
    }
    assert!(trace.contains("transition=Retire"));
    assert!(trace.contains("failure=Receiving cause 2"));
    assert!(!trace.contains("failure=Receiving cause 3"));
    assert_eq!(receiving.omitted_receiving_failures, 1000);
    assert_eq!(dropped.load(Ordering::Acquire), 1000);
    assert_eq!(receiving.report_budget, Some(original));
    assert_eq!(receiving.queue_until, Some(original.until()));
    assert_eq!(receiving.deadlines().collect::<Vec<_>>(), vec![original.until()]);
    drop(receiving);
    assert_eq!(dropped.load(Ordering::Acquire), 1002);
    Ok(())
}
#[test]
fn reporter_omission_count_saturates_without_replacing_retained_errors_or_logging_again()
-> Result<()> {
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut receiving = IncidentReporting::new();
    assert!(
        receiving
            .retain_transition_failure(ReportTransition::CaptureOutput, cause(1, &dropped))
            .is_err()
    );
    receiving.retain_transition_failure(ReportTransition::Retire, cause(2, &dropped))?;
    let first = pointer(receiving.failure().ok_or("First cause missing")?);
    let second = pointer(
        receiving
            .secondary_receiving_failure
            .as_ref()
            .ok_or("Second cause missing")?
            .cause
            .as_ref(),
    );
    receiving.omitted_receiving_failures = u64::MAX - 1;
    let trace = recorded(|| {
        receiving.retain_transition_failure(ReportTransition::Retire, cause(3, &dropped))?;
        receiving.retain_transition_failure(ReportTransition::AdvanceStore, cause(4, &dropped))?;
        Ok(())
    })?;
    assert!(trace.is_empty());
    assert_eq!(receiving.omitted_receiving_failures, u64::MAX);
    assert_eq!(pointer(receiving.failure().ok_or("First cause lost")?), first);
    let retained = receiving.secondary_receiving_failure.as_ref().ok_or("Second cause lost")?;
    assert_eq!(pointer(retained.cause.as_ref()), second);
    assert!(matches!(retained.transition, ReportTransition::Retire));
    assert_eq!(dropped.load(Ordering::Acquire), 2);
    drop(receiving);
    assert_eq!(dropped.load(Ordering::Acquire), 4);
    Ok(())
}
