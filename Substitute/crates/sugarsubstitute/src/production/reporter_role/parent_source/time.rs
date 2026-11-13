// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare original observed monotonic/UTC request facts on the same finite Core receive turn.
use super::*;
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};
use sugarsubstitute_application::incident::{IncidentOperationBudget, IncidentRequestContext};
/// Inert original request timing; retries retain this same domain budget and Core cutoff.
pub(in crate::production) struct ReporterRequestContext {
    pub(in crate::production) request: IncidentRequestContext,
    pub(in crate::production) relay_until: Moment,
}
#[derive(Debug)]
pub(super) enum ReporterTimeCause {
    Clock(ExecutionError),
    Counter(NativeObservationError),
    Utc(SystemTimeError),
    Range(std::num::TryFromIntError),
    Incident(IncidentError),
    Budget(TimeError),
    Wire(DiagnosticWireError),
}
impl std::fmt::Display for ReporterTimeCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reporter original request clock failed")
    }
}
impl std::error::Error for ReporterTimeCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Clock(cause) => cause,
            Self::Counter(cause) => cause,
            Self::Utc(cause) => cause,
            Self::Range(cause) => cause,
            Self::Incident(cause) => cause,
            Self::Budget(cause) => cause,
            Self::Wire(cause) => cause,
        })
    }
}
impl ReporterControlInput {
    /// Encode the retained original control cutoff once on its existing finite Core turn.
    /// A previously frozen counter remains unchanged across queue or native-write retries.
    pub(super) fn prepare_counter(
        &mut self,
        encode: impl FnOnce(Moment) -> Result<CounterDeadline, NativeObservationError>,
    ) -> Result<(), ReporterTimeCause> {
        freeze_counter(&mut self.context.deadline, self.until, encode)
    }
}
fn freeze_counter(
    destination: &mut Option<DiagnosticDeadline>,
    until: Moment,
    encode: impl FnOnce(Moment) -> Result<CounterDeadline, NativeObservationError>,
) -> Result<(), ReporterTimeCause> {
    if destination.is_none() {
        let counter = encode(until).map_err(ReporterTimeCause::Counter)?;
        *destination = Some(
            DiagnosticDeadline::new(counter.ticks(), counter.frequency())
                .map_err(ReporterTimeCause::Wire)?,
        );
    }
    Ok(())
}
pub(super) fn request(
    now: Moment,
    until: Moment,
) -> Result<ReporterRequestContext, ReporterTimeCause> {
    prepare(now, until, SystemTime::now().duration_since(UNIX_EPOCH))
}
fn prepare(
    now: Moment,
    until: Moment,
    elapsed: Result<std::time::Duration, SystemTimeError>,
) -> Result<ReporterRequestContext, ReporterTimeCause> {
    let elapsed = elapsed.map_err(ReporterTimeCause::Utc)?;
    let seconds = i64::try_from(elapsed.as_secs()).map_err(ReporterTimeCause::Range)?;
    let at = IncidentUtc::from_unix_seconds(seconds).map_err(ReporterTimeCause::Incident)?;
    let budget = IncidentOperationBudget::new(now).map_err(ReporterTimeCause::Budget)?;
    Ok(ReporterRequestContext {
        request: IncidentRequestContext::new(budget, at),
        relay_until: until,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_counter_freezes_original_cutoff_and_never_calls_encoder_on_retry()
    -> Result<(), Box<dyn std::error::Error>> {
        let until = Moment::from_nanos(47);
        let counter = CounterDeadline::new(71, 11)?;
        let mut destination = None;
        freeze_counter(&mut destination, until, |actual| {
            assert_eq!(actual, until);
            Ok(counter)
        })?;
        assert_eq!(destination, Some(DiagnosticDeadline::new(71, 11)?));
        freeze_counter(&mut destination, until, |_| {
            Err(NativeObservationError::new(
                "test_counter_retry",
                std::io::Error::other("Encoder must not run again"),
            ))
        })?;
        assert_eq!(destination, Some(DiagnosticDeadline::new(71, 11)?));
        Ok(())
    }
    #[test]
    fn control_counter_failure_preserves_exact_original_cause_without_partial_counter()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut destination = None;
        let result = freeze_counter(&mut destination, Moment::from_nanos(47), |_| {
            Err(NativeObservationError::new(
                "test_counter_error",
                std::io::Error::from_raw_os_error(5),
            ))
        });
        let cause = result.err().ok_or("Missing original encoding cause")?;
        let execution = std::error::Error::source(&cause)
            .and_then(|cause| cause.downcast_ref::<NativeObservationError>())
            .ok_or("Original execution cause changed")?;
        let native = std::error::Error::source(execution)
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .ok_or("Original native cause changed")?;
        assert_eq!(native.raw_os_error(), Some(5));
        assert!(destination.is_none());
        Ok(())
    }
    #[test]
    fn original_utc_cause_and_distinct_core_cutoff_survive_receipt_preparation()
    -> Result<(), Box<dyn std::error::Error>> {
        let now = Moment::from_nanos(19);
        let until = Moment::from_nanos(28);
        let receipt = prepare(now, until, Ok(std::time::Duration::from_secs(50)))?;
        assert_eq!(receipt.request.at().unix_seconds(), 50);
        assert_eq!(receipt.request.budget().started(), now);
        assert_eq!(receipt.request.budget().until(), Moment::from_nanos(60_000_000_019));
        assert_eq!(receipt.relay_until, until);
        let before = UNIX_EPOCH
            .checked_sub(std::time::Duration::from_secs(1))
            .ok_or("no predecessor timestamp")?;
        let cause = prepare(now, until, before.duration_since(UNIX_EPOCH))
            .err()
            .ok_or("original UTC failure missing")?;
        assert!(matches!(cause, ReporterTimeCause::Utc(_)));
        let original = std::error::Error::source(&cause)
            .and_then(|cause| cause.downcast_ref::<SystemTimeError>())
            .ok_or("original UTC cause replaced")?;
        assert_eq!(original.duration(), std::time::Duration::from_secs(1));
        Ok(())
    }
}
