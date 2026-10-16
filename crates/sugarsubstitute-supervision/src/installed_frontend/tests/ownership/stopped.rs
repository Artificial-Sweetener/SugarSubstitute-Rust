// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve genuine execution terminals and partial source custody through explicit closure.
use super::*;
use std::{error::Error, sync::atomic::AtomicBool};
use sugarsubstitute_contracts::execution::{JobOutcome, TerminalKind};

#[derive(Clone, Copy)]
enum Mode {
    Cancel,
    Timeout,
    Failure,
    Panic,
}
struct StoppingSource {
    mode: Mode,
    scope: Arc<Scope>,
    clock: Arc<TestClock>,
    failure: Mutex<Option<FrontendSourceError>>,
    closed: AtomicBool,
}
impl SourcePort for StoppingSource {
    fn inspect(
        &self,
        _: &FrontendInstallationInput,
        state: &mut Inspection,
        _: &JobContext,
    ) -> Result<(), FrontendSourceError> {
        state.manifest = Some(Arc::new(inventory()));
        match self.mode {
            Mode::Cancel => self.scope.cancel(),
            Mode::Timeout => self.clock.0.store(1000, Ordering::Release),
            Mode::Failure => {
                return Err(crate::registry::lock(&self.failure).take().ok_or_else(|| {
                    FrontendSourceError::invalid("missing_original_fixture_cause")
                })?);
            }
            Mode::Panic => {
                std::panic::resume_unwind(Box::new("original inspection panic".to_owned()))
            }
        }
        Ok(())
    }
    fn close(&self, _state: &mut Inspection, _: &JobContext) -> Result<(), FrontendSourceError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
}
fn boundary(
    harness: &mut Harness,
    mode: Mode,
    failure: Option<FrontendSourceError>,
) -> Arc<StoppingSource> {
    let boundary = Arc::new(StoppingSource {
        mode,
        scope: Arc::clone(&harness.scope),
        clock: harness.clock.clone(),
        failure: Mutex::new(failure),
        closed: AtomicBool::new(false),
    });
    harness.owner.port = boundary.clone();
    boundary
}
fn close_partial(harness: &Harness, scope_id: u64, job_id: u64) -> TestResult {
    let cleanup = harness
        .client
        .create_scope(ScopeSpec { id: ScopeId::new(scope_id)?, deadline: None }, None)?;
    let mut job =
        harness.owner.submit_close(&harness.client, &cleanup, Harness::spec(job_id, 2000)?)?;
    harness.wake.until(&harness.client, || matches!(job.status().phase, JobPhase::Terminal(_)))?;
    assert!(matches!(
        job.try_take_completion().ok_or("Missing genuine source close")?.outcome,
        JobOutcome::Success(())
    ));
    assert!(crate::registry::lock(&harness.owner.shared.retained).is_none());
    Ok(())
}

/// Actual stopped jobs retain their original reservation and partial files until explicit cleanup.
#[test]
fn source_terminal_stops_keep_kind_partial_custody_and_charge_after_marker_drop() -> TestResult {
    for (mode, expected) in
        [(Mode::Cancel, TerminalKind::Cancelled), (Mode::Timeout, TerminalKind::TimedOut)]
    {
        let mut harness = Harness::new(false, false)?;
        let native = boundary(&mut harness, mode, None);
        harness.inspect()?;
        let cause = harness.owner.take_inspection().err().ok_or("Actual stop produced a lease")?;
        assert_eq!(cause.terminal_kind(), Some(expected));
        assert!(cause.panic_cause().is_none());
        assert!(
            crate::registry::lock(&harness.owner.shared.retained)
                .as_ref()
                .is_some_and(|state| state.manifest.is_some())
        );
        assert!(!native.closed.load(Ordering::Acquire));
        drop(cause);
        assert_eq!(harness.scope.snapshot().reserved_bytes, FRONTEND_SOURCE_BYTES);
        close_partial(&harness, 2, 2)?;
        assert!(native.closed.load(Ordering::Acquire));
        assert_eq!(harness.scope.snapshot().reserved_bytes, FRONTEND_SOURCE_BYTES);
        drop(harness.owner);
        drop(harness.executor);
        assert_eq!(harness.scope.snapshot().reserved_bytes, 0);
    }
    Ok(())
}

/// Domain failures retain their original cause; panic terminals keep the actual bounded payload.
#[test]
fn source_terminal_failure_and_panic_preserve_original_causes_without_a_lease() -> TestResult {
    for mode in [Mode::Failure, Mode::Panic] {
        let mut harness = Harness::new(false, false)?;
        let original = FrontendSourceError::new(
            "original_inspection_failure",
            std::io::Error::other("original source cause"),
        );
        let pointer = std::ptr::from_ref(original.source().ok_or("Original cause absent")?);
        let native = boundary(&mut harness, mode, Some(original));
        harness.inspect()?;
        let cause = harness.owner.take_inspection().err().ok_or("Actual failure produced lease")?;
        match mode {
            Mode::Failure => {
                assert_eq!(cause.terminal_kind(), None);
                assert!(std::ptr::eq(
                    std::ptr::from_ref(cause.source().ok_or("Returned cause absent")?),
                    pointer
                ));
            }
            Mode::Panic => {
                assert_eq!(cause.terminal_kind(), Some(TerminalKind::Panicked));
                assert_eq!(
                    cause.panic_cause().map(|panic| panic.as_str()),
                    Some("original inspection panic")
                );
            }
            _ => return Err("Unexpected failure fixture mode".into()),
        }
        assert!(!native.closed.load(Ordering::Acquire));
        close_partial(&harness, 2, 2)?;
        assert!(native.closed.load(Ordering::Acquire));
    }
    Ok(())
}

/// A genuine foreign cancellation cannot be classified as this owner's accepted stop.
#[test]
fn source_terminal_foreign_completion_keeps_custody_without_stopped_classification() -> TestResult {
    let mut harness = Harness::new(false, false)?;
    harness.inspect()?;
    let foreign = Arc::new(
        harness.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
    );
    let cancelled = Arc::clone(&foreign);
    let job = harness.client.submit_blocking(&foreign, Harness::spec(3, 1000)?, move |_| {
        cancelled.cancel();
        Ok::<(), FrontendSourceError>(())
    })?;
    harness.owner.pending = Some(job);
    harness.wake.until(&harness.client, || {
        harness
            .owner
            .pending
            .as_ref()
            .is_some_and(|job| matches!(job.status().phase, JobPhase::Terminal(_)))
    })?;
    let cause = harness.owner.take_inspection().err().ok_or("Foreign completion produced lease")?;
    assert_eq!(cause.stage(), "source_delivery_identity");
    assert_eq!(cause.terminal_kind(), None);
    drop(cause);
    assert_eq!(foreign.snapshot().reserved_bytes, FRONTEND_SOURCE_BYTES);
    close_partial(&harness, 3, 4)?;
    drop(harness.owner);
    drop(harness.executor);
    assert_eq!(foreign.snapshot().reserved_bytes, 0);
    Ok(())
}
