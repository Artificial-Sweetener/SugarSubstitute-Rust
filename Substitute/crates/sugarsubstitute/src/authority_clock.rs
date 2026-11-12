// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sample verification time and retain clock failures across synchronous and admitted operations.
use std::{
    error::Error,
    io,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

/// Current UTC seconds, sampled on the admitted verification worker.
fn sample() -> io::Result<i64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?;
    i64::try_from(elapsed.as_secs()).map_err(io::Error::other)
}

/// A single job's clock receipt; poll only after that job's terminal outcome.
#[derive(Clone, Default)]
pub(crate) struct Receipt(Arc<Mutex<Option<io::Error>>>);
impl Receipt {
    /// Preserve the original clock failure while supplying an expired timestamp to verification.
    pub(crate) fn now(&self) -> i64 {
        let mut failure = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        read(&mut failure, &mut sample)
    }
    /// Take the original cause after the worker has stopped accessing this receipt.
    pub(crate) fn take_failure(&self) -> Option<io::Error> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner()).take()
    }
}

/// A failed clock remains failed for every later verification in the same operation.
fn read(failure: &mut Option<io::Error>, sample: &mut impl FnMut() -> io::Result<i64>) -> i64 {
    if failure.is_some() {
        return i64::MAX;
    }
    match sample() {
        Ok(value) => value,
        Err(cause) => {
            *failure = Some(cause);
            i64::MAX
        }
    }
}
/// Re-sample UTC at every verifier callback, preserving a clock failure as the operation cause.
/// An unavailable UTC sample supplies the maximal timestamp so expiry checks fail closed.
pub(crate) fn authorize<T, E: Error + Send + Sync + 'static>(
    operation: impl FnOnce(&mut dyn FnMut() -> i64) -> Result<T, E>,
) -> io::Result<T> {
    with_clock(sample, operation)
}

/// Keep the first clock failure observable even if a later sample would succeed.
fn with_clock<T, E: Error + Send + Sync + 'static>(
    mut sample: impl FnMut() -> io::Result<i64>,
    operation: impl FnOnce(&mut dyn FnMut() -> i64) -> Result<T, E>,
) -> io::Result<T> {
    let mut failure = None;
    let result = operation(&mut || read(&mut failure, &mut sample));
    match failure {
        Some(cause) => Err(cause),
        None => result.map_err(io::Error::other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asynchronous_receipt_keeps_the_first_clock_failure_until_terminal_observation() {
        let receipt = Receipt::default();
        let worker = receipt.clone();
        let mut failure = worker.0.lock().unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(
            read(&mut failure, &mut || Err(io::Error::other("original clock failure"))),
            i64::MAX
        );
        assert_eq!(read(&mut failure, &mut || Ok(100)), i64::MAX);
        drop(failure);
        assert_eq!(
            receipt.take_failure().map(|cause| cause.to_string()).as_deref(),
            Some("original clock failure")
        );
        assert!(receipt.take_failure().is_none());
    }
    #[test]
    fn authority_rechecks_sample_current_time_instead_of_reusing_operation_start() -> io::Result<()>
    {
        let mut value = 50;
        with_clock(
            || {
                value += 1;
                Ok(value)
            },
            |now| {
                assert_eq!(now(), 51);
                assert_eq!(now(), 52);
                Ok::<_, io::Error>(())
            },
        )
    }
    #[test]
    fn failed_clock_remains_expired_and_returns_its_original_error() {
        let mut calls = 0;
        let result = with_clock(
            || {
                calls += 1;
                Err(io::Error::new(io::ErrorKind::InvalidData, "clock unavailable"))
            },
            |now| {
                assert_eq!(now(), i64::MAX);
                assert_eq!(now(), i64::MAX);
                Err::<(), _>(io::Error::other("metadata expired"))
            },
        );
        assert_eq!(calls, 1);
        assert!(result.is_err_and(|cause| cause.kind() == io::ErrorKind::InvalidData
            && cause.to_string() == "clock unavailable"));
    }
}
