// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the original full shutdown transaction, including failed clock measurement.
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
/// Keep one budget or one unresolved original measurement failure on the UI thread.
pub(super) struct ShutdownTransaction {
    state: Option<Result<ShutdownBudget, MeasurementFailure>>,
}
impl ShutdownTransaction {
    /// Keep the role inert until its first genuine shutdown transaction.
    pub(super) const fn new() -> Self {
        Self { state: None }
    }
    /// Adopt an actual earlier child transaction or sample the original configured clock once.
    /// A failed sample remains unresolved; later calls never replace it with a new deadline.
    pub(super) fn request(
        &mut self,
        existing: Option<ShutdownBudget>,
        now: impl FnOnce() -> Result<Moment, Box<dyn Error>>,
    ) -> Result<ShutdownBudget, Box<dyn Error>> {
        if self.state.is_none() {
            let measured =
                existing.map_or_else(|| now().and_then(|now| Ok(ShutdownBudget::new(now)?)), Ok);
            self.state = Some(measured.map_err(|cause| MeasurementFailure(Rc::from(cause))));
        }
        self.tighten(existing);
        self.budget()
    }
    /// Retain a complete earlier actual child transaction without rebuilding grace or total.
    pub(super) fn tighten(&mut self, existing: Option<ShutdownBudget>) {
        if let (Some(Ok(original)), Some(earlier)) = (&mut self.state, existing)
            && earlier.settle_until < original.settle_until
        {
            *original = earlier;
        }
    }
    /// Borrow the latched decision through copied moments or the retained original cause.
    pub(super) fn budget(&self) -> Result<ShutdownBudget, Box<dyn Error>> {
        match &self.state {
            Some(Ok(budget)) => Ok(*budget),
            Some(Err(cause)) => Err(Box::new(cause.clone())),
            None => {
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
    fn first_partial_start_shutdown_uses_one_original_ten_fifteen_second_budget()
    -> Result<(), Box<dyn Error>> {
        let calls = Cell::new(0);
        let mut transaction = ShutdownTransaction::new();
        assert!(transaction.budget().is_err());
        let budget = transaction.request(None, || {
            calls.set(calls.get() + 1);
            Ok(Moment::from_nanos(50))
        })?;
        assert_eq!(budget, ShutdownBudget::new(Moment::from_nanos(50))?);
        assert_eq!(
            transaction.request(None, || {
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
        let original = transaction.request(None, || Ok(Moment::from_nanos(100)))?;
        let earlier = ShutdownBudget::new(Moment::from_nanos(50))?;
        transaction.tighten(Some(earlier));
        assert_eq!(transaction.budget()?, earlier);
        transaction.tighten(Some(original));
        assert_eq!(transaction.budget()?, earlier);
        let mut adopted = ShutdownTransaction::new();
        assert_eq!(
            adopted.request(Some(earlier), || Err("Unexpected clock sample".into()))?,
            earlier
        );
        Ok(())
    }
    #[test]
    fn failed_original_clock_keeps_typed_cause_and_cannot_be_replaced_by_later_measurement()
    -> Result<(), Box<dyn Error>> {
        let calls = Cell::new(0);
        let mut transaction = ShutdownTransaction::new();
        let failure = transaction
            .request(None, || {
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
            .request(Some(ShutdownBudget::new(Moment::from_nanos(1))?), || {
                calls.set(calls.get() + 1);
                Ok(Moment::from_nanos(5))
            })
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
        assert!(transaction.request(None, || Ok(Moment::from_nanos(u64::MAX))).is_err());
        assert!(transaction.request(None, || Ok(Moment::from_nanos(0))).is_err());
        Ok(())
    }
}
