// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise diagnostic producers with genuine admitted contexts and fake native file work.
use super::*;
use crate::installed_frontend::progress::{Phase, Progress, worker_started};
use sugarsubstitute_contracts::execution::JobOutcome;
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};
type Fields = Vec<(String, String)>;
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<Fields>>>);
struct Visitor(Fields);
impl Visit for Visitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push((field.name().to_owned(), format!("{value:?}")));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
    }
}
impl Subscriber for Sink {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target().ends_with("installed_frontend::progress")
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut visitor = Visitor(Vec::new());
        event.record(&mut visitor);
        crate::registry::lock(&self.0).push(visitor.0);
    }
}
struct NativeBoundary {
    original: Arc<Boundary>,
    capture: tracing::Dispatch,
    count: usize,
    fail: bool,
}
impl SourcePort for NativeBoundary {
    fn inspect(
        &self,
        input: &FrontendInstallationInput,
        retained: &mut Inspection,
        context: &JobContext,
    ) -> Result<(), FrontendSourceError> {
        tracing::dispatcher::with_default(&self.capture, || {
            // The same producer consumes the genuine context; local subscribers do not cross worker threads.
            worker_started(context);
            let mut progress = Progress::new(context);
            progress.directory();
            for _ in 0..self.count {
                progress.entry();
                progress.asset();
                progress.bytes(16384);
            }
            if self.fail {
                self.original.clock.0.store(17, Ordering::Release);
                progress.finish(context.now(), retained.handles.len(), false);
                return Err(FrontendSourceError::new(
                    "fake_native_inspection",
                    std::io::Error::other("original-native-cause"),
                ));
            }
            for (index, phase) in [
                Phase::BackendScript,
                Phase::FrontendMetadata,
                Phase::CubeMetadata,
                Phase::FrontendInventory,
                Phase::CubeInventory,
                Phase::Manifest,
            ]
            .into_iter()
            .enumerate()
            {
                self.original.clock.0.store(((index + 1) * 10) as u64, Ordering::Release);
                progress.phase(phase, context.now(), retained.handles.len());
            }
            self.original.inspect(input, retained, context)?;
            self.original.clock.0.store(70, Ordering::Release);
            progress.finish(context.now(), retained.handles.len(), true);
            Ok(())
        })
    }
    fn close(
        &self,
        retained: &mut Inspection,
        context: &JobContext,
    ) -> Result<(), FrontendSourceError> {
        self.original.close(retained, context)
    }
}
fn field<'a>(fields: &'a Fields, name: &str) -> Option<&'a str> {
    fields.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
}
fn exercise(
    count: usize,
    fail: bool,
) -> Result<(Harness, Vec<Fields>), Box<dyn std::error::Error>> {
    let mut harness = Harness::new(false, false)?;
    let sink = Sink::default();
    // Two retained registrars prevent the pinned single-registrar path from sampling an unsubscribed worker.
    // The first is never installed and receives no events; both exist before actual worker entry.
    let _callsite_registry = tracing::Dispatch::new(Sink::default());
    let capture = tracing::Dispatch::new(sink.clone());
    harness.owner.port =
        Arc::new(NativeBoundary { original: harness.boundary.clone(), capture, count, fail });
    harness.inspect()?;
    let events = crate::registry::lock(&sink.0).clone();
    Ok((harness, events))
}
#[test]
fn actual_source_context_reports_original_identity_cutoff_and_monotonic_phase_elapsed() -> TestResult
{
    let (mut harness, events) = exercise(2, false)?;
    assert_eq!(events.len(), 8);
    assert_eq!(field(&events[0], "operation"), Some("frontend_source_worker_started"));
    for event in &events {
        assert_eq!(field(event, "job"), Some("JobId(1)"));
        assert_eq!(field(event, "scope"), Some("ScopeId(1)"));
        assert_eq!(field(event, "original_until"), Some("Some(Moment(1000))"));
    }
    let last = events.last().ok_or("Missing terminal event")?;
    assert_eq!(field(last, "phase"), Some("Manifest"));
    assert_eq!(field(last, "elapsed_ns"), Some("70"));
    assert_eq!(field(last, "phase_elapsed_ns"), Some("10"));
    assert_eq!(field(last, "terminal"), Some("true"));
    let lease = harness.owner.take_inspection()?.ok_or("Actual successful source lease absent")?;
    lease.validate_live()?;
    assert!(matches!(harness.close()?.outcome, JobOutcome::Success(())));
    Ok(())
}
#[test]
fn asset_and_byte_volume_never_adds_per_file_diagnostic_events_or_path_data() -> TestResult {
    let (mut harness, events) = exercise(4096, false)?;
    assert_eq!(events.len(), 8);
    let last = events.last().ok_or("Missing terminal event")?;
    for key in ["assets", "entries"] {
        assert_eq!(field(last, key), Some("4096"));
    }
    assert_eq!(field(last, "bytes"), Some("67108864"));
    for event in &events {
        assert!(
            !event.iter().any(|(key, value)| key.contains("path") || value.contains("fixture"))
        );
    }
    let _lease = harness.owner.take_inspection()?.ok_or("Actual source lease absent")?;
    assert!(matches!(harness.close()?.outcome, JobOutcome::Success(())));
    Ok(())
}
#[test]
fn native_failure_keeps_original_cause_partial_counters_and_original_deadline() -> TestResult {
    use std::error::Error;
    let (mut harness, events) = exercise(3, true)?;
    assert_eq!(events.len(), 2);
    let last = events.last().ok_or("Missing terminal event")?;
    assert_eq!(field(last, "phase"), Some("SelectedObjects"));
    assert_eq!(field(last, "succeeded"), Some("false"));
    assert_eq!(field(last, "entries"), Some("3"));
    assert_eq!(field(last, "elapsed_ns"), Some("17"));
    let cause = match harness.owner.take_inspection() {
        Err(cause) => cause,
        Ok(_) => return Err("Original failure lost".into()),
    };
    assert_eq!(cause.stage(), "fake_native_inspection");
    assert_eq!(
        cause.source().ok_or("Original native cause absent")?.to_string(),
        "original-native-cause"
    );
    assert!(matches!(harness.close()?.outcome, JobOutcome::Success(())));
    Ok(())
}
