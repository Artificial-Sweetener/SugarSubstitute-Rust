// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only native calls while exercising the production original-cutoff and cause owner.
use super::*;
use std::{cell::Cell, error::Error};
struct Boundary {
    ticks: i64,
    frequency: i64,
    transfers: usize,
    statuses: usize,
    copy_fails: bool,
    status_fails: bool,
}
impl Boundary {
    fn at(ticks: i64, frequency: i64) -> Self {
        Self { ticks, frequency, transfers: 0, statuses: 0, copy_fails: false, status_fails: false }
    }
}
impl CopyBoundary for Boundary {
    fn prepare(&mut self) -> Result<(), ReportSurfaceError> {
        Ok(())
    }
    fn counter(&mut self) -> Result<(i64, i64), ReportSurfaceError> {
        Ok((self.ticks, self.frequency))
    }
    fn transfer(&mut self) -> Result<(), ReportSurfaceError> {
        self.transfers += 1;
        if self.copy_fails {
            Err(ReportSurfaceError::new(
                "original_clipboard_transfer",
                std::io::Error::from_raw_os_error(5),
            ))
        } else {
            Ok(())
        }
    }
    fn status(&mut self, _: bool) -> Result<(), ReportSurfaceError> {
        self.statuses += 1;
        if self.status_fails {
            Err(ReportSurfaceError::rejected("later_native_status"))
        } else {
            Ok(())
        }
    }
}
#[test]
fn original_counter_rejects_delayed_delivery_exact_expiry_and_frequency_mismatch()
-> Result<(), Box<dyn Error>> {
    let original = CounterDeadline::new(100, 10)?;
    for (ticks, frequency) in [(100, 10), (101, 10), (99, 11), (0, 10), (-1, 10)] {
        let mut boundary = Boundary::at(ticks, frequency);
        let mut first = None;
        let observed = Cell::new(false);
        assert!(
            execute(&mut boundary, original, Ok(()), &mut first, |_| {
                observed.set(true);
                Ok(())
            })
            .is_err()
        );
        assert_eq!(boundary.transfers, 0);
        assert_eq!(boundary.statuses, 0);
        assert!(!observed.get());
        assert!(first.is_none());
        assert_eq!(original, CounterDeadline::new(100, 10)?);
    }
    let mut boundary = Boundary::at(99, 10);
    let mut first = None;
    execute(&mut boundary, original, Ok(()), &mut first, |outcome| {
        assert!(outcome.is_ok());
        Ok(())
    })?;
    assert_eq!(boundary.transfers, 1);
    assert_eq!(boundary.statuses, 1);
    assert!(first.is_none());
    Ok(())
}
#[test]
fn actual_copy_cause_survives_later_native_status_or_observer_failure() -> Result<(), Box<dyn Error>>
{
    for status_fails in [false, true] {
        let mut boundary = Boundary { copy_fails: true, status_fails, ..Boundary::at(99, 10) };
        let mut first = None;
        let observed = Cell::new(false);
        let later =
            execute(&mut boundary, CounterDeadline::new(100, 10)?, Ok(()), &mut first, |outcome| {
                observed.set(true);
                assert_eq!(outcome, Err(ReportSurfaceFailure::Clipboard));
                Err(ReportSurfaceError::rejected("later_copy_observer"))
            })
            .err()
            .ok_or("later failure missing")?;
        assert_eq!(
            later.operation(),
            if status_fails { "later_native_status" } else { "later_copy_observer" }
        );
        let cause = first.as_ref().ok_or("original clipboard cause missing")?;
        assert_eq!(cause.operation(), "original_clipboard_transfer");
        assert_eq!(
            cause
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .and_then(std::io::Error::raw_os_error),
            Some(5)
        );
        assert_eq!(boundary.transfers, 1);
        assert_eq!(observed.get(), !status_fails);
    }
    Ok(())
}
#[test]
fn rejected_host_copy_performs_no_clipboard_transfer() -> Result<(), Box<dyn Error>> {
    let mut boundary = Boundary::at(99, 10);
    let mut first = None;
    execute(
        &mut boundary,
        CounterDeadline::new(100, 10)?,
        Err(ReportSurfaceFailure::Store),
        &mut first,
        |outcome| {
            assert_eq!(outcome, Err(ReportSurfaceFailure::Clipboard));
            Ok(())
        },
    )?;
    assert_eq!(boundary.transfers, 0);
    assert_eq!(
        first.as_ref().ok_or("original denial missing")?.operation(),
        "report_copy_host_rejected"
    );
    Ok(())
}
