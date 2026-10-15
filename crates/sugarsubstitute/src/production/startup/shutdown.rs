// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the original full shutdown transaction, including failed clock measurement.
use super::super::child::ShutdownInitiation;
use crate::production::close_diagnostic::{BudgetStage, CloseTrace};
use std::{error::Error, fmt, rc::Rc};
use sugarsubstitute_contracts::{execution::Moment, lifecycle::ShutdownBudget};

/// Share the actual first failure without retrying its clock or changing its type chain.
#[derive(Clone)]
struct MeasurementFailure(Rc<dyn Error>);
impl fmt::Debug for MeasurementFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MeasurementFailure")
    }
}
impl fmt::Display for MeasurementFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original startup shutdown measurement failed")
    }
}
impl Error for MeasurementFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}
/// Distinguish an in-flight external observation from its immutable measured result.
enum State {
    CapturePending,
    Measured(Result<ShutdownBudget, MeasurementFailure>),
}
/// Keep one capture observation or original local-clock result on the UI thread.
pub(super) struct ShutdownTransaction {
    state: Option<State>,
}
impl ShutdownTransaction {
    /// Keep the role inert until its first genuine shutdown transaction.
    pub(super) const fn new() -> Self {
        Self { state: None }
    }
    /// Borrow facts from this owner's actual first retained measurement cause only.
    pub(super) fn failure_diagnostic(
        &self,
    ) -> Option<(
        sugarsubstitute_desktop::StartupCloseStage,
        sugarsubstitute_desktop::StartupCaptureState,
        sugarsubstitute_desktop::StartupCauseCode,
    )> {
        match &self.state {
            Some(State::Measured(Err(cause))) => {
                super::close_diagnostic::measurement_diagnostic(cause.0.as_ref())
            }
            _ => None,
        }
    }
    /// Observe pending original capture measurement without granting settlement or a budget.
    pub(super) fn pending(&self) -> bool {
        matches!(self.state, Some(State::CapturePending))
    }
    /// Adopt an actual earlier child transaction or sample the original configured clock once.
    /// A failed sample remains unresolved; later calls never replace it with a new deadline.
    pub(super) fn request(
        &mut self,
        origin: ShutdownInitiation,
        now: impl FnOnce() -> Result<Moment, Box<dyn Error>>,
    ) -> Result<Option<ShutdownBudget>, Box<dyn Error>> {
        let existing = match origin {
            ShutdownInitiation::Captured(budget) => Some(budget),
            _ => None,
        };
        if self.state.is_none() || matches!(self.state, Some(State::CapturePending)) {
            match origin {
                ShutdownInitiation::CapturePending => {
                    if self.state.is_none() {
                        CloseTrace::Budget { stage: BudgetStage::CapturePending, moments: None }
                            .emit();
                    }
                    self.state = Some(State::CapturePending);
                    return Ok(None);
                }
                ShutdownInitiation::NoCapture if self.state.is_some() => return Ok(None),
                ShutdownInitiation::NoCapture => {
                    let measured = now().and_then(|now| Ok(ShutdownBudget::new(now)?));
                    match &measured {
                        Ok(budget) => {
                            CloseTrace::Budget {
                                stage: BudgetStage::LocalMeasured,
                                moments: Some([
                                    budget.started.as_nanos(),
                                    budget.graceful_until.as_nanos(),
                                    budget.settle_until.as_nanos(),
                                ]),
                            }
                            .emit();
                        }
                        Err(_) => CloseTrace::Budget {
                            stage: BudgetStage::MeasurementFailed,
                            moments: None,
                        }
                        .emit(),
                    }
                    self.state = Some(State::Measured(
                        measured.map_err(|cause| MeasurementFailure(Rc::from(cause))),
                    ));
                }
                ShutdownInitiation::Captured(budget) => {
                    self.state = Some(State::Measured(Ok(budget)));
                    CloseTrace::Budget {
                        stage: BudgetStage::CaptureLatched,
                        moments: Some([
                            budget.started.as_nanos(),
                            budget.graceful_until.as_nanos(),
                            budget.settle_until.as_nanos(),
                        ]),
                    }
                    .emit();
                }
            }
        }
        self.tighten(existing);
        self.budget().map(Some)
    }
    /// Retain the actual failed capture observation; pending data is never sent here.
    pub(super) fn reject(&mut self, cause: Box<dyn Error>) -> Box<dyn Error> {
        if self.state.is_none() || matches!(self.state, Some(State::CapturePending)) {
            let failure = MeasurementFailure(Rc::from(cause));
            self.state = Some(State::Measured(Err(failure.clone())));
            CloseTrace::Budget { stage: BudgetStage::CaptureMeasurementFailed, moments: None }
                .emit();
            Box::new(failure)
        } else {
            cause
        }
    }
    /// Retain a complete earlier actual child transaction without rebuilding grace or total.
    pub(super) fn tighten(&mut self, existing: Option<ShutdownBudget>) {
        if let (Some(State::Measured(Ok(original))), Some(earlier)) = (&mut self.state, existing)
            && earlier.settle_until < original.settle_until
        {
            *original = earlier;
        }
    }
    /// Borrow the latched decision through copied moments or the retained original cause.
    pub(super) fn budget(&self) -> Result<ShutdownBudget, Box<dyn Error>> {
        match &self.state {
            Some(State::Measured(Ok(budget))) => Ok(*budget),
            Some(State::Measured(Err(cause))) => Err(Box::new(cause.clone())),
            None | Some(State::CapturePending) => {
                Err(std::io::Error::other("Original startup shutdown transaction absent").into())
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn retained_startup_close_diagnostic_survives_measurement_rejection_without_resampling()
    -> Result<(), Box<dyn Error>> {
        use sugarsubstitute_desktop::{
            StartupCaptureState, StartupCauseCode, StartupCloseFailure, StartupCloseStage,
            StartupNativeCause,
        };
        let mut transaction = ShutdownTransaction::new();
        let original = sugarsubstitute_execution::windows::NativeObservationError::new(
            "capture_cleanup_clock",
            std::io::Error::other("private original boundary"),
        );
        let cause = StartupCloseFailure::retain(
            StartupCloseStage::CaptureObservation,
            StartupCaptureState::ClockUnavailable,
            StartupCauseCode::Native(StartupNativeCause::CaptureClock, None),
            Box::new(original),
        );
        let rejected = transaction.reject(cause);
        assert_eq!(
            transaction.failure_diagnostic(),
            Some((
                StartupCloseStage::CaptureObservation,
                StartupCaptureState::ClockUnavailable,
                StartupCauseCode::Native(StartupNativeCause::CaptureClock, None)
            ))
        );
        let samples = Cell::new(0);
        assert!(
            transaction
                .request(ShutdownInitiation::NoCapture, || {
                    samples.set(samples.get() + 1);
                    Ok(Moment::from_nanos(100))
                })
                .is_err()
        );
        assert_eq!(samples.get(), 0);
        let close = rejected
            .source()
            .and_then(|cause| cause.downcast_ref::<StartupCloseFailure>())
            .ok_or("Retained first stage absent")?;
        assert_eq!(
            close
                .original()
                .downcast_ref::<sugarsubstitute_execution::windows::NativeObservationError>()
                .ok_or("Original native cause lost")?
                .stage(),
            "capture_cleanup_clock"
        );
        assert!(transaction.budget().is_err());
        Ok(())
    }
    #[test]
    fn first_partial_start_shutdown_uses_one_original_ten_fifteen_second_budget()
    -> Result<(), Box<dyn Error>> {
        let calls = Cell::new(0);
        let mut transaction = ShutdownTransaction::new();
        assert!(transaction.budget().is_err());
        let budget = transaction.request(ShutdownInitiation::NoCapture, || {
            calls.set(calls.get() + 1);
            Ok(Moment::from_nanos(50))
        })?;
        assert_eq!(budget, Some(ShutdownBudget::new(Moment::from_nanos(50))?));
        assert_eq!(
            transaction.request(ShutdownInitiation::NoCapture, || {
                calls.set(calls.get() + 1);
                Ok(Moment::from_nanos(500))
            })?,
            budget
        );
        assert_eq!(calls.get(), 1);
        Ok(())
    }
    #[test]
    fn earlier_actual_child_transaction_keeps_all_three_moments_and_later_cannot_renew()
    -> Result<(), Box<dyn Error>> {
        let mut transaction = ShutdownTransaction::new();
        let original = transaction
            .request(ShutdownInitiation::NoCapture, || Ok(Moment::from_nanos(100)))?
            .ok_or("Original budget missing")?;
        let earlier = ShutdownBudget::new(Moment::from_nanos(50))?;
        transaction.tighten(Some(earlier));
        assert_eq!(transaction.budget()?, earlier);
        transaction.tighten(Some(original));
        assert_eq!(transaction.budget()?, earlier);
        let mut adopted = ShutdownTransaction::new();
        assert_eq!(
            adopted.request(ShutdownInitiation::Captured(earlier), || Err(
                "Unexpected clock sample".into()
            ))?,
            Some(earlier)
        );
        Ok(())
    }
    #[test]
    fn failed_original_clock_keeps_typed_cause_and_cannot_be_replaced_by_later_measurement()
    -> Result<(), Box<dyn Error>> {
        let calls = Cell::new(0);
        let mut transaction = ShutdownTransaction::new();
        let failure = transaction
            .request(ShutdownInitiation::NoCapture, || {
                calls.set(calls.get() + 1);
                Err(Box::new(std::io::Error::other("first clock")))
            })
            .err()
            .ok_or("Failed clock accepted")?;
        assert_eq!(
            failure
                .source()
                .and_then(|e| e.downcast_ref::<std::io::Error>())
                .ok_or("Original type lost")?
                .to_string(),
            "first clock"
        );
        let repeated = transaction
            .request(
                ShutdownInitiation::Captured(ShutdownBudget::new(Moment::from_nanos(1))?),
                || {
                    calls.set(calls.get() + 1);
                    Ok(Moment::from_nanos(5))
                },
            )
            .err()
            .ok_or("Failure replaced")?;
        assert_eq!(
            repeated
                .source()
                .and_then(|e| e.downcast_ref::<std::io::Error>())
                .ok_or("Repeated type lost")?
                .to_string(),
            "first clock"
        );
        assert_eq!(calls.get(), 1);
        Ok(())
    }
    #[test]
    fn overflow_is_unresolved_and_does_not_adopt_a_new_interval() -> Result<(), Box<dyn Error>> {
        let mut transaction = ShutdownTransaction::new();
        assert!(
            transaction
                .request(ShutdownInitiation::NoCapture, || Ok(Moment::from_nanos(u64::MAX)))
                .is_err()
        );
        assert!(
            transaction
                .request(ShutdownInitiation::NoCapture, || Ok(Moment::from_nanos(0)))
                .is_err()
        );
        Ok(())
    }
    #[test]
    fn capture_pending_does_not_sample_another_clock_and_adopts_only_original_budget()
    -> Result<(), Box<dyn Error>> {
        let calls = Cell::new(0);
        let mut transaction = ShutdownTransaction::new();
        for origin in [ShutdownInitiation::CapturePending, ShutdownInitiation::NoCapture] {
            assert_eq!(
                transaction.request(origin, || {
                    calls.set(calls.get() + 1);
                    Err("A pending capture cannot sample the UI clock".into())
                })?,
                None
            );
            assert!(transaction.pending());
            assert!(transaction.budget().is_err());
        }
        let original = ShutdownBudget::new(Moment::from_nanos(50))?;
        assert_eq!(
            transaction.request(ShutdownInitiation::Captured(original), || {
                calls.set(calls.get() + 1);
                Ok(Moment::from_nanos(1_000))
            })?,
            Some(original)
        );
        assert!(!transaction.pending());
        assert_eq!(transaction.budget()?, original);
        assert_eq!(
            transaction.request(ShutdownInitiation::NoCapture, || {
                calls.set(calls.get() + 1);
                Ok(Moment::from_nanos(2_000))
            })?,
            Some(original)
        );
        assert_eq!(calls.get(), 0);
        Ok(())
    }
    #[test]
    fn actual_capture_clock_failure_after_pending_remains_original_and_cannot_renew()
    -> Result<(), Box<dyn Error>> {
        let mut transaction = ShutdownTransaction::new();
        assert_eq!(
            transaction.request(ShutdownInitiation::CapturePending, || {
                Err("Pending capture cannot sample another clock".into())
            })?,
            None
        );
        let original = Box::new(std::io::Error::other("actual capture clock"));
        let failure = transaction.reject(original);
        let retained = failure
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .ok_or("Original clock type lost")?;
        assert_eq!(retained.to_string(), "actual capture clock");
        let address = std::ptr::from_ref(retained);
        let repeated = transaction
            .request(
                ShutdownInitiation::Captured(ShutdownBudget::new(Moment::from_nanos(1))?),
                || Err("Failed measurement cannot sample another clock".into()),
            )
            .err()
            .ok_or("Actual failure replaced")?;
        assert_eq!(
            std::ptr::from_ref(
                repeated
                    .source()
                    .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                    .ok_or("Retained failure type lost")?
            ),
            address
        );
        assert!(!transaction.pending());
        Ok(())
    }
}

#[cfg(test)]
mod transition_diagnostic_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    type Result<T> = std::result::Result<T, Box<dyn Error>>;
    #[derive(Clone)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Sink {
        type Writer = Self;
        fn make_writer(&'a self) -> Self {
            self.clone()
        }
    }
    impl std::io::Write for Sink {
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
        let subscriber = tracing_subscriber::fmt()
            .with_writer(Sink(bytes.clone()))
            .without_time()
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, emit)?;
        let bytes = bytes.lock().map_err(|_| "Trace fixture poisoned")?.clone();
        Ok(String::from_utf8(bytes)?)
    }

    #[test]
    fn shutdown_budget_logs_only_real_pending_to_latched_transition_without_resampling()
    -> Result<()> {
        let mut owner = ShutdownTransaction::new();
        let original = ShutdownBudget::new(Moment::from_nanos(50))?;
        let calls = std::cell::Cell::new(0);
        let log = recorded(|| {
            for origin in [
                ShutdownInitiation::CapturePending,
                ShutdownInitiation::CapturePending,
                ShutdownInitiation::NoCapture,
                ShutdownInitiation::Captured(original),
                ShutdownInitiation::Captured(original),
                ShutdownInitiation::NoCapture,
            ] {
                let _observed = owner.request(origin, || {
                    calls.set(calls.get() + 1);
                    Ok(Moment::from_nanos(999))
                })?;
            }
            Ok(())
        })?;
        assert_eq!(calls.get(), 0);
        assert_eq!(owner.budget()?, original);
        assert_eq!(log.matches("operation=\"child_shutdown_budget\"").count(), 2);
        assert_eq!(log.matches("stage=CapturePending").count(), 1);
        assert_eq!(log.matches("stage=CaptureLatched").count(), 1);
        assert!(!log.contains("LocalMeasured"));
        Ok(())
    }
}
