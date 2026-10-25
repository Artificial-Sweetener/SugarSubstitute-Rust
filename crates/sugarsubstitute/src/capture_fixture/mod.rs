// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify one real capture registration and explicit teardown through public owners.
//! The isolated naming facts grant no authenticated authority. This cohort observes
//! neither desktop connection nor a fault, dump, incident/report or symbolization.
mod activity;
mod failure;
mod lifetime;
mod options;
mod receipt;
mod specification;
mod state;
#[cfg(test)]
mod tests;

pub(crate) use options::Options;
use state::{CleanupDemand, Evidence, StopPlan};
use std::{error::Error, num::NonZeroUsize, path::PathBuf, sync::Arc, time::Instant};
use sugarsubstitute_contracts::{
    crash::{CaptureBudget, CaptureObservation, CaptureState},
    execution::*,
    lifecycle::{CoordinatorKey, ShutdownBudget},
};
use sugarsubstitute_crash::{CaptureStartInput, CrashAdmissionError, WindowsCrashCapture};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, run_headless};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock, WorkerStatus,
    windows::{NativeObservationError, ProcessExit},
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const MAX_RECEIPT_BYTES: usize = 16 * 1024;

/// Hold bounded diagnostics only after the native pump and execution teardown.
pub(crate) struct FixtureResult {
    path: PathBuf,
    receipt: serde_json::Value,
    outcome: Result,
}
impl FixtureResult {
    /// Let the host persist post-teardown evidence while preserving the first cause.
    /// The supplied writer owns filesystem effects; this consumer introduces none.
    pub(crate) fn finish_with(self, writer: impl FnOnce(PathBuf, Vec<u8>) -> Result) -> Result {
        let written = serde_json::to_vec_pretty(&self.receipt)
            .map_err(|e| Box::new(e) as Box<dyn Error>)
            .and_then(|bytes| {
                if bytes.len() > MAX_RECEIPT_BYTES {
                    return Err("Capture receipt bound exceeded".into());
                }
                writer(self.path, bytes)
            });
        failure::combine(self.outcome, written)
    }
}

/// Run on the native main thread, preserving owners through all evidence failures.
/// Only the external runner may prepare protected storage or supervise an uncertain
/// process beyond the original bounds. A deadline does not certify native retirement.
pub(crate) fn run(options: Options) -> Result<FixtureResult> {
    let mut fixture = Fixture::new(options)?;
    if let Err(cause) = run_headless(&mut fixture) {
        fixture.failures.record(Box::new(cause));
    }
    let passed = fixture.passed();
    let receipt = fixture.receipt(passed);
    let outcome = if passed {
        Ok(())
    } else {
        if fixture.failures.is_empty() {
            fixture.failures.record("Capture evidence incomplete".into());
        }
        Err(Box::new(fixture.failures) as Box<dyn Error>)
    };
    Ok(FixtureResult { path: fixture.options.receipt, receipt, outcome })
}

/// Retain the original registration, cleanup and execution owners through errors.
struct Fixture {
    options: Options,
    input: Option<CaptureStartInput>,
    clock: Arc<MonotonicClock>,
    budget: CaptureBudget,
    registration_alarm: Instant,
    registration_expired: bool,
    capture: WindowsCrashCapture,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    start_scope: Option<Scope>,
    cleanup_scope: Option<Scope>,
    native_scope: Option<Scope>,
    rejected_watch: Option<OwnedJob<ProcessExit, NativeObservationError>>,
    start_admitted: bool,
    start_terminal_seen: bool,
    closing: bool,
    stop: Option<StopPlan>,
    demand: CleanupDemand,
    evidence: Evidence,
    failures: failure::Failures,
    reported_capture_errors: usize,
    execution_stopping: bool,
    workers: Option<WorkerStatus>,
    finished: bool,
    retained_bytes_at_finish: Option<u64>,
}
impl Fixture {
    /// Prepare lexical inputs and immutable clock bounds before native startup.
    fn new(mut options: Options) -> Result<Self> {
        let clock = MonotonicClock::new();
        let budget = CaptureBudget::new(clock.now())?;
        let before = clock.now();
        let native = Instant::now();
        let after = clock.now();
        let registration_alarm = state::map_deadline(budget.until(), before, native, after)?;
        let input = options.input.take().ok_or("Capture fixture input missing")?;
        let key = options.coordinator;
        Ok(Self {
            options,
            input: Some(input),
            clock,
            budget,
            registration_alarm,
            registration_expired: false,
            capture: WindowsCrashCapture::new(key),
            owner: None,
            client: None,
            start_scope: None,
            cleanup_scope: None,
            native_scope: None,
            rejected_watch: None,
            start_admitted: false,
            start_terminal_seen: false,
            closing: false,
            stop: None,
            demand: CleanupDemand::new(),
            evidence: Evidence::new(key),
            failures: failure::Failures::new(),
            reported_capture_errors: 0,
            execution_stopping: false,
            workers: None,
            finished: false,
            retained_bytes_at_finish: None,
        })
    }
    /// Accept the complete normal cohort only from actual capture and worker facts.
    fn passed(&self) -> bool {
        let snapshot = self.capture.snapshot();
        self.finished
            && self.start_admitted
            && self.start_terminal_seen
            && self.failures.is_empty()
            && self.evidence.complete()
            && snapshot.state == CaptureState::Closed
            && snapshot.handler_exit == Some(1)
            && state::registration_retention_matches(
                snapshot.callback_pin_bytes,
                snapshot.callback_pin_reservation_bytes,
                self.retained_bytes_at_finish,
            )
            && self.workers.is_some_and(|w| w.async_finished && w.cpu_finished && w.native_finished)
    }
}
