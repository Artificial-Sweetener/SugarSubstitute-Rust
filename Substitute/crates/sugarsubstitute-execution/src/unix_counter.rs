// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own the finite `CLOCK_MONOTONIC` query that Unix native peers share with this process.
use crate::native::NativeObservationError;

/// The counter counts nanoseconds, so its fixed frequency is one billion per second.
const FREQUENCY: u64 = 1_000_000_000;

pub(crate) fn sample() -> Result<(u64, u64), NativeObservationError> {
    let mut now = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: CLOCK_MONOTONIC is a valid clock id and the call writes only this live timespec.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0 {
        return Err(NativeObservationError::new(
            "execution_monotonic_counter",
            std::io::Error::last_os_error(),
        ));
    }
    let ticks = u64::try_from(now.tv_sec)
        .ok()
        .and_then(|seconds| seconds.checked_mul(FREQUENCY))
        .zip(u64::try_from(now.tv_nsec).ok())
        .and_then(|(seconds, nanos)| seconds.checked_add(nanos))
        .filter(|ticks| *ticks > 0)
        .ok_or_else(|| {
            NativeObservationError::new(
                "execution_positive_counter",
                std::io::Error::other("CLOCK_MONOTONIC returned a nonpositive or overflowing time"),
            )
        })?;
    Ok((ticks, FREQUENCY))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared counter is positive, nanosecond-based and never runs backwards.
    #[test]
    fn monotonic_counter_is_positive_and_never_decreases() -> Result<(), NativeObservationError> {
        let (first, frequency) = sample()?;
        let (second, _) = sample()?;
        assert_eq!(frequency, FREQUENCY);
        assert!(first > 0 && second >= first);
        Ok(())
    }
}
