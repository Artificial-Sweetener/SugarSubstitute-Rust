// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inject only handle APIs into the actual adoption/cleanup algorithm.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Boundary {
    inherited: [bool; 4],
    invalid_event: Option<usize>,
    fail_duplicate: Option<usize>,
    fail_close: Option<usize>,
    closed: Vec<usize>,
}
impl Boundary {
    fn new() -> Self {
        Self {
            inherited: [true; 4],
            invalid_event: None,
            fail_duplicate: None,
            fail_close: None,
            closed: Vec::new(),
        }
    }
}
fn error(stage: &'static str) -> NativeObservationError {
    NativeObservationError::new(stage, std::io::Error::from_raw_os_error(6))
}
impl Operations for Boundary {
    fn flags(&mut self, raw: usize) -> Result<u32, NativeObservationError> {
        Ok(if raw > 4 || self.inherited[raw - 1] { HANDLE_FLAG_INHERIT.0 } else { 0 })
    }
    fn clear_inherit(&mut self, raw: usize) -> Result<(), NativeObservationError> {
        self.inherited[raw - 1] = false;
        Ok(())
    }
    fn validate_event(&mut self, raw: usize) -> Result<(), NativeObservationError> {
        if Some(raw) == self.invalid_event { Err(error("invalid_event")) } else { Ok(()) }
    }
    fn duplicate(&mut self, raw: usize, output: &mut usize) -> Result<(), NativeObservationError> {
        *output = raw + 4;
        if Some(raw) == self.fail_duplicate { Err(error("partial_duplicate")) } else { Ok(()) }
    }
    fn close(&mut self, raw: usize) -> Result<(), NativeObservationError> {
        if Some(raw) == self.fail_close {
            return Err(error("controlled_close"));
        }
        self.closed.push(raw);
        Ok(())
    }
}
fn lease() -> Lease {
    Lease { originals: [Some(1), Some(2), Some(3), Some(4)], duplicates: [None; 4] }
}
#[test]
fn forwarding_handle_algorithm_retains_partial_duplicate_and_failed_close() -> TestResult {
    let mut lease = lease();
    let mut boundary = Boundary::new();
    boundary.fail_duplicate = Some(3);
    assert_eq!(
        lease.prepare_with(&mut boundary).err().map(|cause| cause.stage()),
        Some("partial_duplicate")
    );
    assert_eq!(lease.duplicates, [Some(5), Some(6), Some(7), None]);
    boundary.fail_close = Some(6);
    assert!(lease.retire_with(&mut boundary).is_err());
    assert_eq!(lease.duplicates, [None, Some(6), None, None]);
    assert_eq!(lease.originals, [None; 4]);
    assert_eq!(boundary.closed, [5, 7, 1, 2, 3, 4]);
    boundary.fail_close = None;
    lease.retire_with(&mut boundary)?;
    assert_eq!(boundary.closed, [5, 7, 1, 2, 3, 4, 6]);
    Ok(())
}
#[test]
fn forwarding_handle_algorithm_rejects_wrong_objects_before_duplicate() -> TestResult {
    let mut lease = lease();
    let mut boundary = Boundary::new();
    boundary.invalid_event = Some(1);
    assert_eq!(
        lease.prepare_with(&mut boundary).err().map(|cause| cause.stage()),
        Some("invalid_event")
    );
    assert_eq!(lease.duplicates, [None; 4]);
    lease.retire_with(&mut boundary)?;
    assert_eq!(boundary.closed, [1, 2, 3, 4]);
    Ok(())
}
#[test]
fn forwarding_handle_algorithm_clears_all_originals_and_pins_exact_four_duplicates() -> TestResult {
    let mut lease = lease();
    let mut boundary = Boundary::new();
    assert_eq!(lease.prepare_with(&mut boundary)?, [5, 6, 7, 8]);
    assert_eq!(boundary.inherited, [false; 4]);
    lease.retire_with(&mut boundary)?;
    assert_eq!(boundary.closed, [5, 6, 7, 8, 1, 2, 3, 4]);
    Ok(())
}
#[test]
fn forwarding_handle_algorithm_rejects_noninherited_input() -> TestResult {
    let mut lease = lease();
    let mut boundary = Boundary::new();
    boundary.inherited[0] = false;
    assert_eq!(
        lease.prepare_with(&mut boundary).err().map(|cause| cause.stage()),
        Some("forwarding_original_flags")
    );
    lease.retire_with(&mut boundary)?;
    Ok(())
}
