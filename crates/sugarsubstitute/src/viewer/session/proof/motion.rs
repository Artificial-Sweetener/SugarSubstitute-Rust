// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Generate deterministic qualification motion without owning clocks, waits or navigation policy.

use std::time::Duration;
use sugarsubstitute_canvas::ScrollPhase;

/// One bounded wheel observation from the proof motion sequence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MotionStep {
    /// Change in the base-1.25 zoom exponent since the last emitted observation.
    pub(super) steps: f64,
    /// Explicit gesture phase for the emitted observation.
    pub(super) phase: ScrollPhase,
}

/// Invalid proof-motion configuration or elapsed-time observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MotionError {
    /// A zero sampling period cannot define a cadence.
    ZeroPeriod,
    /// A zero duration cannot define a motion sequence.
    ZeroDuration,
    /// Duration arithmetic cannot represent the motion endpoint or a sample.
    TimeOverflow,
    /// Elapsed time moved behind the preceding observation.
    ClockRegression,
}

impl std::fmt::Display for MotionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ZeroPeriod => "Proof motion period must be positive",
            Self::ZeroDuration => "Proof motion duration must be positive",
            Self::TimeOverflow => "Proof motion exceeded elapsed time",
            Self::ClockRegression => "Proof motion clock moved backwards",
        })
    }
}

impl std::error::Error for MotionError {}

/// Pure qualification motion sampled from caller-supplied elapsed monotonic time.
///
/// Its cumulative base-1.25 zoom exponent is `0.75 * (1 - cos(2π * progress))`: zero
/// at both endpoints and 1.5 at the midpoint. Emitted wheel deltas are positive on the outward
/// half and negative on the return half.
///
/// This owner emits normalized wheel observations only. `Navigation` retains camera animation and
/// zoom semantics, while the native event loop retains deadline scheduling and input delivery.
pub(super) struct Motion {
    start: Duration,
    period: Duration,
    duration: Duration,
    end: Duration,
    deadline: Option<Duration>,
    last_now: Option<Duration>,
    sampled: Option<Duration>,
    cumulative: f64,
    began: bool,
    finished: bool,
}

impl Motion {
    /// Create an inert motion beginning at `start` with an exact finite endpoint.
    ///
    /// # Errors
    /// Rejects zero period/duration or an endpoint outside the representable `Duration` range.
    pub(super) fn new(
        start: Duration,
        period: Duration,
        duration: Duration,
    ) -> Result<Self, MotionError> {
        if period.is_zero() {
            return Err(MotionError::ZeroPeriod);
        }
        if duration.is_zero() {
            return Err(MotionError::ZeroDuration);
        }
        let end = start.checked_add(duration).ok_or(MotionError::TimeOverflow)?;
        Ok(Self {
            start,
            period,
            duration,
            end,
            deadline: Some(start),
            last_now: None,
            sampled: None,
            cumulative: 0.0,
            began: false,
            finished: false,
        })
    }

    /// Emit at most one observation for the newest due sample.
    ///
    /// Late calls skip older cadence slots and evaluate the current elapsed progress directly.
    /// The terminal `End` observation returns the cumulative exponent exactly to zero.
    ///
    /// # Errors
    /// Rejects backward elapsed time or unrepresentable cadence arithmetic without mutation.
    pub(super) fn next(&mut self, now: Duration) -> Result<Option<MotionStep>, MotionError> {
        if self.last_now.is_some_and(|previous| now < previous) {
            return Err(MotionError::ClockRegression);
        }
        if self.finished {
            self.last_now = Some(now);
            return Ok(None);
        }
        if now < self.start {
            self.last_now = Some(now);
            return Ok(None);
        }

        let sample = if now >= self.end { self.end } else { self.latest_sample(now)? };
        if self.sampled == Some(sample) {
            self.last_now = Some(now);
            return Ok(None);
        }
        let phase = if sample == self.end {
            ScrollPhase::End
        } else if self.began {
            ScrollPhase::Update
        } else {
            ScrollPhase::Begin
        };
        let target = if phase == ScrollPhase::End { 0.0 } else { self.exponent(sample)? };
        let steps =
            if phase == ScrollPhase::End { -self.cumulative } else { target - self.cumulative };
        let next_deadline = if phase == ScrollPhase::End {
            None
        } else {
            let remaining = self.end.checked_sub(sample).ok_or(MotionError::ClockRegression)?;
            Some(if self.period >= remaining {
                self.end
            } else {
                sample.checked_add(self.period).ok_or(MotionError::TimeOverflow)?
            })
        };

        self.cumulative += steps;
        if phase == ScrollPhase::End {
            self.cumulative = 0.0;
            self.finished = true;
        } else {
            self.began = true;
        }
        self.sampled = Some(sample);
        self.deadline = next_deadline;
        self.last_now = Some(now);
        Ok(Some(MotionStep { steps, phase }))
    }

    /// Next desired elapsed-clock sample, including an overdue sample not yet observed by `next`.
    pub(super) const fn next_deadline(&self) -> Option<Duration> {
        self.deadline
    }

    /// Whether the exact terminal `End` observation was emitted.
    pub(super) const fn finished(&self) -> bool {
        self.finished
    }

    fn latest_sample(&self, now: Duration) -> Result<Duration, MotionError> {
        let elapsed = now.checked_sub(self.start).ok_or(MotionError::ClockRegression)?;
        let intervals = elapsed.as_nanos() / self.period.as_nanos();
        let offset = checked_multiply(self.period, intervals)?;
        self.start.checked_add(offset).ok_or(MotionError::TimeOverflow)
    }

    fn exponent(&self, sample: Duration) -> Result<f64, MotionError> {
        let elapsed = sample.checked_sub(self.start).ok_or(MotionError::ClockRegression)?;
        let progress = elapsed.as_secs_f64() / self.duration.as_secs_f64();
        Ok(0.75 * (1.0 - (std::f64::consts::TAU * progress).cos()))
    }
}

fn checked_multiply(period: Duration, multiplier: u128) -> Result<Duration, MotionError> {
    let nanoseconds = period.as_nanos().checked_mul(multiplier).ok_or(MotionError::TimeOverflow)?;
    let seconds = nanoseconds / 1_000_000_000;
    let subsecond = (nanoseconds % 1_000_000_000) as u32;
    let seconds = u64::try_from(seconds).map_err(|_| MotionError::TimeOverflow)?;
    Ok(Duration::new(seconds, subsecond))
}

#[cfg(test)]
#[path = "motion/tests.rs"]
mod tests;
