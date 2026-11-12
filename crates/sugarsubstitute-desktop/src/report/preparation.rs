// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare the authentic complete report before Tao while retaining the same execution wake.
use super::*;
use crate::bootstrap::{Bootstrap, WaitOutcome};
use std::{
    error::Error,
    fmt,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{execution::CompletionWake, incident::ReportCommand};

/// Prepare complete charged text using the existing execution owner before native presentation.
pub trait ReportSessionPreparation: ReportSessionContent {
    /// Consume bounded actual completions and return options only after authenticated whole-view transfer.
    /// Parsing and filesystem work belong to finite execution workers.
    /// # Errors
    /// Preserve the original failure and remain shutdown-capable after partial admission.
    fn prepare(
        &mut self,
        maximum: NonZeroUsize,
    ) -> Result<Option<ReportSurfaceOptions>, ReportSurfaceError>;
    /// Borrow the next genuine original operation cutoff in the native `Instant` epoch.
    /// A past cutoff must be consumed by `prepare`, never silently extended.
    fn preparation_deadline(&self) -> Option<Instant>;
}

/// Start one owner, prepare its authentic whole view, then hand the same wake to the native surface.
/// No application window or Tao loop exists during preparation. Native waits pump bounded messages.
/// # Errors
/// Retain original preparation/cleanup causes and uncertain owner custody. Expiry grants no settlement.
/// Reject oversized Session/content metadata before bootstrap creation or content start.
pub fn run_prepared_report_surface<C: ReportSessionPreparation + 'static>(
    mut content: C,
) -> Result<ReportSurfaceReceipt, ReportSurfaceError> {
    super::surface::validate_content_capacity::<Prepared<C>>()?;
    let bootstrap = Bootstrap::new()?;
    let mut failure = Failures::default();
    if let Err(cause) = call(|| content.start(bootstrap.wake())) {
        retain(&mut failure, cause);
    }
    let mut closing = None;
    loop {
        if let Err(cause) = bootstrap.begin_turn() {
            retain(&mut failure, cause.into());
        }
        if failure.first.is_none() {
            match call(|| content.prepare(NonZeroUsize::MIN.saturating_add(31))) {
                Ok(Some(options)) => {
                    return run_report_surface(options, Prepared { content, bootstrap });
                }
                Ok(None) => {}
                Err(cause) => retain(&mut failure, cause),
            }
        }
        if failure.first.is_some() {
            if closing.is_none() {
                closing = Instant::now().checked_add(Duration::from_secs(15));
                if let Err(cause) = call(|| content.request_shutdown()) {
                    retain(&mut failure, cause);
                }
            }
            match call(|| content.poll_shutdown()) {
                Ok(true) => {
                    let mut native_failure = None;
                    bootstrap.retain_close_failure(&mut native_failure);
                    if let Some(cause) = native_failure {
                        retain(&mut failure, cause.into());
                    }
                    if let Err(cause) = call(|| content.finish_shutdown()) {
                        retain(&mut failure, cause);
                    }
                    return Err(ReportSurfaceError::new("report_preparation_failure", failure));
                }
                Ok(false) => {}
                Err(cause) => retain(&mut failure, cause),
            }
            if closing.is_none_or(|until| Instant::now() >= until) {
                return Err(ReportSurfaceError::new(
                    "report_preparation_unsettled",
                    Unsettled { cause: failure, content: Some(content), bootstrap },
                ));
            }
        }
        let until = if let Some(closing) = closing {
            Some(
                Instant::now()
                    .checked_add(Duration::from_millis(10))
                    .map_or(closing, |cadence| cadence.min(closing)),
            )
        } else {
            match call(|| Ok(content.preparation_deadline())) {
                Ok(until) => until,
                Err(cause) => {
                    retain(&mut failure, cause);
                    continue;
                }
            }
        };
        match bootstrap.wait(until) {
            Ok(WaitOutcome::Woken | WaitOutcome::Deadline) => {}
            Ok(WaitOutcome::Quit(_)) => {
                retain(&mut failure, ReportSurfaceError::rejected("report_preparation_quit"))
            }
            Err(cause) => retain(&mut failure, cause.into()),
        }
    }
}

fn call<T>(
    operation: impl FnOnce() -> Result<T, ReportSurfaceError>,
) -> Result<T, ReportSurfaceError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)).map_err(|payload| {
        ReportSurfaceError::new("report_preparation_panic", super::error::PanicCause::new(payload))
    })?
}

#[derive(Default)]
struct Failures {
    first: Option<ReportSurfaceError>,
    secondary: [Option<ReportSurfaceError>; 2],
    omitted: u64,
}
fn retain(primary: &mut Failures, cause: ReportSurfaceError) {
    if primary.first.is_none() {
        primary.first = Some(cause);
    } else if let Some(slot) = primary.secondary.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(cause);
    } else {
        primary.omitted = primary.omitted.saturating_add(1);
        tracing::error!(
            operation = cause.operation(),
            omitted = primary.omitted,
            "Additional Reporter preparation failure exceeds retained cause slots"
        );
    }
}

impl fmt::Debug for Failures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReportPreparationFailures")
            .field("primary", &self.first)
            .field("secondary", &self.secondary)
            .field("omitted", &self.omitted)
            .finish()
    }
}
impl fmt::Display for Failures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Reporter preparation failed with retained original causes")
    }
}
impl Error for Failures {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.first.as_ref().map(|cause| cause as _)
    }
}
struct Unsettled<C> {
    cause: Failures,
    content: Option<C>,
    bootstrap: Bootstrap,
}
impl<C> fmt::Debug for Unsettled<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UnsettledReportPreparation")
    }
}
impl<C> fmt::Display for Unsettled<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Reporter preparation retains unsettled native custody")
    }
}
impl<C: 'static> Error for Unsettled<C> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
impl<C> Drop for Unsettled<C> {
    fn drop(&mut self) {
        // The same unresolved content keeps its workers, frames and original credit through process exit.
        if let Some(content) = self.content.take() {
            std::mem::forget(content);
        }
        let _ = &self.bootstrap;
    }
}
struct Prepared<C> {
    content: C,
    bootstrap: Bootstrap,
}
impl<C: ReportSessionContent> ReportSessionContent for Prepared<C> {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), ReportSurfaceError> {
        self.bootstrap.handoff(wake).map_err(Into::into)
    }
    fn poll(&mut self, maximum: NonZeroUsize) -> Result<ReportSurfaceUpdate, ReportSurfaceError> {
        self.content.poll(maximum)
    }
    fn action(
        &mut self,
        request: ReportRequestId,
        command: ReportCommand,
    ) -> Result<(), ReportSurfaceError> {
        self.content.action(request, command)
    }
    fn observe(&mut self, observation: ReportSurfaceObservation) -> Result<(), ReportSurfaceError> {
        self.content.observe(observation)
    }
    fn request_shutdown(&mut self) -> Result<(), ReportSurfaceError> {
        self.content.request_shutdown()
    }
    fn poll_shutdown(&mut self) -> Result<bool, ReportSurfaceError> {
        self.content.poll_shutdown()
    }
    fn finish_shutdown(&mut self) -> Result<(), ReportSurfaceError> {
        let mut failure = Failures::default();
        if let Err(cause) = self.content.finish_shutdown() {
            retain(&mut failure, cause);
        }
        if let Err(cause) = self.bootstrap.disconnect() {
            retain(&mut failure, cause.into());
        }
        if failure.first.is_none() {
            Ok(())
        } else {
            Err(ReportSurfaceError::new("report_preparation_cleanup", failure))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug)]
    struct Cause(u32);
    impl fmt::Display for Cause {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "cause {}", self.0)
        }
    }
    impl Error for Cause {}
    #[test]
    fn repeated_cleanup_failure_keeps_first_cause_and_bounded_distinct_secondaries() {
        let mut failures = Failures::default();
        for id in 1..=5 {
            retain(&mut failures, ReportSurfaceError::new("prepare_fixture", Cause(id)));
        }
        assert_eq!(
            failures
                .first
                .as_ref()
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<Cause>())
                .map(|cause| cause.0),
            Some(1)
        );
        assert_eq!(failures.secondary.iter().flatten().count(), 2);
        assert_eq!(failures.omitted, 2);
    }
    #[test]
    fn preparation_callback_unwind_preserves_original_payload_without_disclosing_it() {
        let result = call::<()>(|| {
            std::panic::resume_unwind(Box::new(String::from("private failure payload")))
        });
        let cause = result.err();
        assert_eq!(
            cause.as_ref().map(ReportSurfaceError::operation),
            Some("report_preparation_panic")
        );
        assert!(
            cause
                .as_ref()
                .is_some_and(|cause| !cause.to_string().contains("private failure payload"))
        );
        assert!(cause.as_ref().and_then(Error::source).is_some());
    }
}
