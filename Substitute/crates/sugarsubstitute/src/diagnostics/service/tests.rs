// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify routing, overload, original memory custody and actual native sink retirement.
use super::*;
use crate::{completion_wake::wait_while_watched, paused_clock::PausedClock};
use std::{
    error::Error,
    io::Write,
    path::PathBuf,
    sync::{
        Weak,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::MemoryClaim;

#[derive(Default)]
struct Observation {
    entered: bool,
    released: bool,
    events: Vec<String>,
    omitted: u64,
}
#[derive(Default)]
struct Controlled {
    state: Mutex<Observation>,
    changed: Condvar,
    path: Option<PathBuf>,
}
impl Controlled {
    /// Block until the actual sink has been entered by its publication worker.
    fn entered(&self) -> Result<(), Box<dyn Error>> {
        let state = self.state.lock().map_err(|_| "Diagnostic fixture poisoned")?;
        drop(wait_while_watched(
            &self.changed,
            state,
            || "the actual diagnostic sink to be entered".to_owned(),
            |state| !state.entered,
        ));
        Ok(())
    }
    fn release(&self) -> Result<(), Box<dyn Error>> {
        self.state.lock().map_err(|_| "Diagnostic fixture poisoned")?.released = true;
        self.changed.notify_all();
        Ok(())
    }
}
impl Sink for Controlled {
    fn overloaded(&self, cause: &io::Error) -> bool {
        self.path.is_some()
            && sugarsubstitute_supervision::diagnostic_storage::operational::writer_busy(cause)
    }
    fn append(&self, frames: &[String], omitted: u64, context: &JobContext) -> io::Result<()> {
        {
            let mut state =
                self.state.lock().map_err(|_| io::Error::other("Diagnostic fixture poisoned"))?;
            state.entered = true;
            self.changed.notify_all();
            let mut state = wait_while_watched(
                &self.changed,
                state,
                || "the test to release the diagnostic sink".to_owned(),
                |state| !state.released,
            );
            state.events.extend_from_slice(frames);
            state.omitted = state.omitted.saturating_add(omitted);
        }
        if let Some(path) = &self.path {
            sugarsubstitute_supervision::diagnostic_storage::operational::fixture::append(
                path, frames, omitted, context,
            )?;
        }
        Ok(())
    }
}
#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("Fixture output poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn native_route_preserves_correlation_redacts_copies_and_releases_every_directory_pin()
-> Result<(), Box<dyn Error>> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("operational");
    let sink = Arc::new(Controlled { path: Some(path.clone()), ..Controlled::default() });
    let health = DiagnosticHealth::fixture();
    let service = Service::start(health.clone(), sink.clone(), PausedClock::new())?;
    let queue = service.queue();
    queue.push("operation=fixture_bootstrap\n".into());
    sink.entered()?;
    sink.release()?;
    let output = Output::default();
    let writer = output.clone();
    let subscriber = crate::diagnostics::subscriber(
        health.clone(),
        move || writer.clone(),
        "info".into(),
        Some(queue.clone()),
    );
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(
            operation = "correlation_fixture",
            request_id = 42_u64,
            document_id = "document_3",
            workflow_id = "workflow_4",
            execution_id = 999999_u64,
            connection_id = 7_u64,
            cause = "token=secret",
            credential = "secret",
            "Structured host event"
        );
    });
    service.finish(&health);
    health.check()?;
    assert!(queue.state().memory.is_none());
    let state = sink.state.lock().map_err(|_| "Diagnostic fixture poisoned")?;
    let frame = state
        .events
        .iter()
        .find(|frame| frame.contains("correlation_fixture"))
        .ok_or("Routed correlation event absent")?;
    for expected in [
        "request_id=42",
        "document_id=\"document_3\"",
        "workflow_id=\"workflow_4\"",
        "execution_id=999999",
        "connection_id=7",
    ] {
        assert!(frame.contains(expected), "missing {expected}: {frame}");
    }
    assert!(!frame.contains("secret"));
    assert!(frame.contains("cause=[redacted]"));
    let stderr = output.0.lock().map_err(|_| "Fixture output poisoned")?;
    assert_eq!(std::str::from_utf8(&stderr)?, frame);
    let pages: Vec<_> = std::fs::read_dir(&path)?.collect::<io::Result<_>>()?;
    assert!(pages.len() <= 10);
    let mut persisted = false;
    for entry in pages {
        if entry.file_name().to_string_lossy().starts_with("page-") {
            let bytes = std::fs::read(entry.path())?;
            assert!(bytes.len() <= 65_536);
            let page: serde_json::Value = serde_json::from_slice(&bytes)?;
            persisted |= page["events"]
                .as_array()
                .is_some_and(|events| events.iter().any(|value| value.as_str() == Some(frame)));
        }
    }
    assert!(persisted);
    std::fs::rename(&path, temporary.path().join("retired"))?;
    Ok(())
}
#[test]
fn overloaded_queue_rejects_new_frames_counts_exact_loss_and_retires_original_charge()
-> Result<(), Box<dyn Error>> {
    let sink = Arc::new(Controlled::default());
    let health = DiagnosticHealth::fixture();
    let service = Service::start(health.clone(), sink.clone(), PausedClock::new())?;
    let queue = service.queue();
    queue.push("operation=bootstrap\n".into());
    sink.entered()?;
    let claim: Weak<MemoryClaim> =
        Arc::downgrade(queue.state().memory.as_ref().ok_or("Original queue memory absent")?);
    let observed = service.owner.poll_shutdown()?;
    assert_eq!(observed.unresolved_jobs.len(), 2);
    assert_eq!(
        observed.unresolved_jobs.iter().map(|job| job.spec.reservation_bytes).sum::<u64>(),
        PUMP_MEMORY_BYTES + BATCH_MEMORY_BYTES
    );
    let limits = service.owner.limits();
    assert_eq!(limits.memory_bytes, 256 * 1024 * 1024);
    assert_eq!((limits.async_workers, limits.blocking_workers, limits.native_workers), (2, 2, 1));
    assert!(matches!(limits.cpu_workers, 1 | 2));
    queue.push("x".repeat(EVENT_BYTES + 1));
    for sequence in 0..100 {
        queue.push(format!("operation=overload sequence={sequence}\n"));
    }
    assert!(claim.upgrade().is_some());
    sink.release()?;
    service.finish(&health);
    health.check()?;
    let state = sink.state.lock().map_err(|_| "Diagnostic fixture poisoned")?;
    assert_eq!(state.events.len() as u64 + state.omitted, 102);
    assert!(state.events.iter().all(|frame| frame.len() <= EVENT_BYTES));
    assert!(state.omitted >= 36);
    assert!(state.events.len() <= 65);
    assert!(claim.upgrade().is_none());
    Ok(())
}
#[test]
fn cancelled_pump_keeps_drained_frame_memory_until_the_actual_sink_returns()
-> Result<(), Box<dyn Error>> {
    let sink = Arc::new(Controlled::default());
    let health = DiagnosticHealth::fixture();
    let service = Service::start(health.clone(), sink.clone(), PausedClock::new())?;
    let queue = service.queue();
    queue.push("operation=bootstrap\n".into());
    sink.entered()?;
    let claim =
        Arc::downgrade(queue.state().memory.as_ref().ok_or("Original queue memory absent")?);
    queue.state().clear();
    service.job.cancel();
    assert!(claim.upgrade().is_some());
    sink.release()?;
    service.finish(&health);
    assert!(claim.upgrade().is_none());
    assert!(health.check().is_err());
    Ok(())
}
#[derive(Debug)]
struct Original(Arc<AtomicUsize>);
impl std::fmt::Display for Original {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Original persistent sink failure")
    }
}
impl Error for Original {}
impl Drop for Original {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}
struct Failed(Arc<AtomicUsize>);
impl Sink for Failed {
    fn append(&self, _: &[String], _: u64, _: &JobContext) -> io::Result<()> {
        Err(io::Error::other(Original(self.0.clone())))
    }
}
#[test]
fn persistent_sink_failure_retains_its_original_source_until_the_last_health_observer()
-> Result<(), Box<dyn Error>> {
    let dropped = Arc::new(AtomicUsize::new(0));
    let health = DiagnosticHealth::fixture();
    let service =
        Service::start(health.clone(), Arc::new(Failed(dropped.clone())), PausedClock::new())?;
    service.queue().push("operation=failure\n".into());
    service.finish(&health);
    let failure = health.check().err().ok_or("Persistent sink failure was hidden")?;
    assert!(
        failure
            .source()
            .and_then(|cause| cause.downcast_ref::<io::Error>())
            .and_then(io::Error::get_ref)
            .is_some_and(|cause| cause.is::<Original>())
    );
    drop(health);
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    drop(failure);
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    Ok(())
}

#[derive(Default)]
struct BusyThenReady {
    calls: AtomicUsize,
    observed: Mutex<Option<(Vec<String>, u64)>>,
}
impl Sink for BusyThenReady {
    fn append(&self, frames: &[String], omitted: u64, _: &JobContext) -> io::Result<()> {
        if self.calls.fetch_add(1, Ordering::AcqRel) == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        *self.observed.lock().map_err(|_| io::Error::other("Fixture state poisoned"))? =
            Some((frames.to_vec(), omitted));
        Ok(())
    }
    fn overloaded(&self, cause: &io::Error) -> bool {
        cause.kind() == io::ErrorKind::WouldBlock
    }
}
#[test]
fn explicit_writer_contention_retries_the_original_batch_without_reclassifying_sink_failures()
-> Result<(), Box<dyn Error>> {
    let sink = Arc::new(BusyThenReady::default());
    let health = DiagnosticHealth::fixture();
    let service = Service::start(health.clone(), sink.clone(), PausedClock::new())?;
    service.queue().push("operation=contention\n".into());
    service.finish(&health);
    health.check()?;
    assert_eq!(sink.calls.load(Ordering::Acquire), 2);
    let observed = sink.observed.lock().map_err(|_| "Fixture state poisoned")?;
    let (frames, omitted) = observed.as_ref().ok_or("Actual retried batch absent")?;
    assert_eq!(frames.len() as u64 + omitted, 1);
    Ok(())
}

/// Execution time that a test advances past any former elapsed-time bound without waiting.
struct AdvancedClock {
    actual: Arc<sugarsubstitute_execution::MonotonicClock>,
    offset: std::sync::atomic::AtomicU64,
}
impl AdvancedClock {
    fn advance(&self, by: Duration) {
        self.offset.fetch_add(u64::try_from(by.as_nanos()).unwrap_or(u64::MAX), Ordering::AcqRel);
    }
}
impl Clock for AdvancedClock {
    fn now(&self) -> sugarsubstitute_contracts::execution::Moment {
        sugarsubstitute_contracts::execution::Moment::from_nanos(
            self.actual.now().as_nanos().saturating_add(self.offset.load(Ordering::Acquire)),
        )
    }
    fn sleep_until(
        &self,
        deadline: sugarsubstitute_contracts::execution::Moment,
    ) -> sugarsubstitute_contracts::execution::WaitFuture {
        self.actual.sleep_until(sugarsubstitute_contracts::execution::Moment::from_nanos(
            deadline.as_nanos().saturating_sub(self.offset.load(Ordering::Acquire)),
        ))
    }
}
/// A history writer that another process holds until the test releases it.
#[derive(Default)]
struct HeldWriter {
    state: Mutex<HeldState>,
    changed: Condvar,
}
#[derive(Default)]
struct HeldState {
    released: bool,
    latest_busy_attempt: Option<sugarsubstitute_contracts::execution::Moment>,
    published: Vec<String>,
}
impl Sink for HeldWriter {
    fn append(&self, frames: &[String], _: u64, context: &JobContext) -> io::Result<()> {
        let mut state =
            self.state.lock().map_err(|_| io::Error::other("Fixture state poisoned"))?;
        if !state.released {
            state.latest_busy_attempt = Some(context.now());
            self.changed.notify_all();
            return Err(io::ErrorKind::WouldBlock.into());
        }
        state.published.extend_from_slice(frames);
        Ok(())
    }
    fn overloaded(&self, cause: &io::Error) -> bool {
        cause.kind() == io::ErrorKind::WouldBlock
    }
}
impl HeldWriter {
    /// Block until publication retries the busy writer at or after `moment`.
    fn busy_at_or_after(
        &self,
        moment: sugarsubstitute_contracts::execution::Moment,
    ) -> Result<(), Box<dyn Error>> {
        let state = self.state.lock().map_err(|_| "Fixture state poisoned")?;
        drop(wait_while_watched(
            &self.changed,
            state,
            || "publication to retry the busy history writer".to_owned(),
            |state| state.latest_busy_attempt.is_none_or(|attempt| attempt < moment),
        ));
        Ok(())
    }
}
#[test]
fn a_writer_held_past_any_elapsed_time_delays_the_batch_without_failing_diagnostics()
-> Result<(), Box<dyn Error>> {
    let clock = Arc::new(AdvancedClock {
        actual: sugarsubstitute_execution::MonotonicClock::new(),
        offset: Default::default(),
    });
    let sink = Arc::new(HeldWriter::default());
    let health = DiagnosticHealth::fixture();
    let service = Service::start(health.clone(), sink.clone(), clock.clone())?;
    service.queue().push("operation=held_writer\n".into());
    sink.busy_at_or_after(sugarsubstitute_contracts::execution::Moment::from_nanos(0))?;
    clock.advance(Duration::from_secs(3_600));
    let later = clock.now();
    sink.busy_at_or_after(later)?;
    health.check()?;
    sink.state.lock().map_err(|_| "Fixture state poisoned")?.released = true;
    service.finish(&health);
    health.check()?;
    let state = sink.state.lock().map_err(|_| "Fixture state poisoned")?;
    assert_eq!(state.published, ["operation=held_writer\n"]);
    Ok(())
}

#[test]
fn a_writer_held_through_shutdown_loses_the_last_batch_without_failing_the_process()
-> Result<(), Box<dyn Error>> {
    let sink = Arc::new(HeldWriter::default());
    let health = DiagnosticHealth::fixture();
    let service = Service::start(health.clone(), sink.clone(), PausedClock::new())?;
    let queue = service.queue();
    queue.push("operation=held_at_close\n".into());
    sink.busy_at_or_after(sugarsubstitute_contracts::execution::Moment::from_nanos(0))?;
    // Another process keeps the writer through this process's whole close: the shutdown cutoff
    // has passed while the batch still waits behind it.
    queue.stop(sugarsubstitute_contracts::execution::Moment::from_nanos(0));
    service.finish(&health);
    health.check()?;
    assert!(sink.state.lock().map_err(|_| "Fixture state poisoned")?.published.is_empty());
    Ok(())
}
