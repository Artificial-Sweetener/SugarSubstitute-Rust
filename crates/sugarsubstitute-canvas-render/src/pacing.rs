// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound canvas presentation and retain portable evidence about its observed cadence.

use std::time::Duration;
use sugarsubstitute_canvas::FrameStamp;
use sugarsubstitute_contracts::execution::SurfaceId;

/// Whether a native frame contains the canonical camera or a transient projection of it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationMode {
    /// The native frame was prepared directly for its displayed camera.
    Canonical,
    /// The native frame projects retained canonical content for responsive interaction.
    Projected,
}

/// Work currently retaining the pacer's sole presentation slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationStage {
    /// Frame preparation was submitted and has not produced a native frame.
    Preparing,
    /// A completed native frame is waiting for native presentation.
    AwaitingNative,
}

/// Current admission decision without consuming a display slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationReadiness {
    /// One current frame may be submitted immediately.
    Ready,
    /// The sole slot is available at this exact elapsed-clock deadline.
    WaitingUntil(Duration),
    /// One exact frame still retains preparation or native presentation custody.
    Busy {
        /// Exact retained frame identity.
        target: FrameStamp,
        /// Current custody stage.
        stage: PresentationStage,
    },
}

/// Bounded aggregate duration evidence without retaining individual samples.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimingSummary {
    /// Number of observed intervals, saturating rather than wrapping.
    pub samples: u64,
    /// Saturating sum of all observed intervals.
    pub total: Duration,
    /// Largest observed interval.
    pub maximum: Duration,
}

impl TimingSummary {
    fn observe(&mut self, value: Duration) {
        self.samples = self.samples.saturating_add(1);
        self.total = self.total.saturating_add(value);
        self.maximum = self.maximum.max(value);
    }
}

/// Compact process-lifetime presentation observations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PresentationStats {
    /// Frames admitted for preparation.
    pub submitted: u64,
    /// Submitted frames abandoned before native queue acceptance.
    pub abandoned: u64,
    /// Cadence slots deliberately skipped to consume only the newest due slot.
    pub skipped_slots: u64,
    /// Canonical frames accepted by the native presentation queue.
    pub canonical_accepted: u64,
    /// Projected frames accepted by the native presentation queue.
    pub projected_accepted: u64,
    /// Most recent elapsed-clock time at which the native queue accepted a frame.
    pub last_native_acceptance: Option<Duration>,
    /// Time from the originating input/intent observation to native queue acceptance.
    pub input_to_native: TimingSummary,
    /// Time between consecutive native queue acceptances.
    pub native_acceptance_cadence: TimingSummary,
}

/// Invalid pacing observation rejected without changing presentation custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacingError {
    /// A zero display period cannot define a cadence.
    ZeroPeriod,
    /// Elapsed time moved behind the last accepted pacing observation.
    ClockRegression,
    /// Duration arithmetic cannot represent the next cadence slot.
    TimeOverflow,
    /// The originating input was observed after its frame submission.
    InputAfterSubmission,
    /// Submission arrived before its current cadence slot.
    TooEarly {
        /// Exact elapsed-clock deadline for the next slot.
        deadline: Duration,
    },
    /// Another frame retains the sole presentation slot.
    Busy {
        /// Exact retained frame identity.
        target: FrameStamp,
        /// Current custody stage.
        stage: PresentationStage,
    },
    /// No frame retains presentation custody, including after an acknowledgement was consumed.
    NoPendingPresentation,
    /// An observation named a frame other than the one retaining custody.
    ForeignTarget {
        /// Exact frame retaining custody.
        expected: FrameStamp,
        /// Frame named by the rejected observation.
        observed: FrameStamp,
    },
    /// An observation is not valid in the retained frame's current stage.
    WrongStage {
        /// Stage required by the observation.
        expected: PresentationStage,
        /// Current retained custody stage.
        actual: PresentationStage,
    },
    /// One pacer cannot accept work for another native surface.
    ForeignSurface,
}

impl std::fmt::Display for PacingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroPeriod => formatter.write_str("Presentation period must be positive"),
            Self::ClockRegression => formatter.write_str("Presentation clock moved backwards"),
            Self::TimeOverflow => formatter.write_str("Presentation cadence exceeded elapsed time"),
            Self::InputAfterSubmission => {
                formatter.write_str("Presentation input was observed after submission")
            }
            Self::TooEarly { deadline } => {
                write!(formatter, "Presentation is not due until {deadline:?}")
            }
            Self::Busy { target, stage } => {
                write!(formatter, "Presentation {target:?} is still {stage:?}")
            }
            Self::NoPendingPresentation => formatter.write_str("No presentation is pending"),
            Self::ForeignTarget { expected, observed } => write!(
                formatter,
                "Presentation observation {observed:?} does not match {expected:?}",
            ),
            Self::WrongStage { expected, actual } => {
                write!(formatter, "Presentation stage {actual:?} does not match {expected:?}",)
            }
            Self::ForeignSurface => formatter.write_str("Presentation belongs to another surface"),
        }
    }
}

impl std::error::Error for PacingError {}

#[derive(Clone, Copy, Debug)]
struct PendingPresentation {
    target: FrameStamp,
    mode: PresentationMode,
    input_at: Duration,
    stage: PresentationStage,
}

/// Portable owner of one pacing cadence and at most one presentation in flight.
///
/// Callers supply elapsed monotonic time from the execution clock and retain all jobs, timers,
/// native handles and event-loop affinity. Camera animation remains owned by `Navigation`.
#[derive(Debug)]
pub struct PresentationPacer {
    period: Duration,
    anchor: Option<Duration>,
    next_slot: Option<Duration>,
    last_observation: Option<Duration>,
    surface: Option<SurfaceId>,
    pending: Option<PendingPresentation>,
    stats: PresentationStats,
}

impl PresentationPacer {
    /// Create an idle pacing owner. Its first frame is immediately eligible.
    ///
    /// # Errors
    /// Returns [`PacingError::ZeroPeriod`] for a zero display period.
    pub fn new(period: Duration) -> Result<Self, PacingError> {
        if period.is_zero() {
            return Err(PacingError::ZeroPeriod);
        }
        Ok(Self {
            period,
            anchor: None,
            next_slot: None,
            last_observation: None,
            surface: None,
            pending: None,
            stats: PresentationStats::default(),
        })
    }

    /// Current fractional display period used for admission.
    pub const fn period(&self) -> Duration {
        self.period
    }

    /// Update monitor timing while preserving the established cadence epoch and frame custody.
    /// This does not inspect or reset camera animation.
    ///
    /// # Errors
    /// Rejects a zero period, backward elapsed time or an unrepresentable future slot.
    pub fn set_frame_period(&mut self, period: Duration, now: Duration) -> Result<(), PacingError> {
        self.validate_time(now)?;
        if period.is_zero() {
            return Err(PacingError::ZeroPeriod);
        }
        let next_slot =
            self.anchor.map(|anchor| next_slot_after(anchor, period, now)).transpose()?;
        self.period = period;
        self.next_slot = next_slot;
        self.last_observation = Some(now);
        Ok(())
    }

    /// Inspect whether work may be submitted without reserving the slot.
    ///
    /// # Errors
    /// Rejects elapsed time older than the last accepted pacing observation.
    pub fn readiness(&self, now: Duration) -> Result<PresentationReadiness, PacingError> {
        self.validate_time(now)?;
        if let Some(pending) = self.pending {
            return Ok(PresentationReadiness::Busy {
                target: pending.target,
                stage: pending.stage,
            });
        }
        Ok(match self.next_slot {
            Some(deadline) if now < deadline => PresentationReadiness::WaitingUntil(deadline),
            _ => PresentationReadiness::Ready,
        })
    }

    /// Earliest future pacing wake while idle. Busy or already-due work supplies no timer.
    ///
    /// # Errors
    /// Rejects elapsed time older than the last accepted pacing observation.
    pub fn next_deadline(&self, now: Duration) -> Result<Option<Duration>, PacingError> {
        self.validate_time(now)?;
        Ok(if self.pending.is_some() {
            None
        } else {
            self.next_slot.filter(|deadline| *deadline > now)
        })
    }

    /// Reserve the sole slot after the caller submits preparation for this exact frame.
    /// Late submissions skip every obsolete slot and keep the original cadence phase.
    ///
    /// `input_at` identifies the input or redraw intent represented by the frame. It is retained
    /// until native acceptance to measure end-to-end input-to-native-submission latency.
    ///
    /// # Errors
    /// Rejects backward time, early or concurrent submission, foreign identities,
    /// invalid input ordering and cadence overflow without changing state.
    pub fn submitted(
        &mut self,
        target: FrameStamp,
        mode: PresentationMode,
        input_at: Duration,
        now: Duration,
    ) -> Result<(), PacingError> {
        self.validate_time(now)?;
        if input_at > now {
            return Err(PacingError::InputAfterSubmission);
        }
        if let Some(pending) = self.pending {
            return Err(PacingError::Busy { target: pending.target, stage: pending.stage });
        }
        if self.surface.is_some_and(|surface| surface != target.surface) {
            return Err(PacingError::ForeignSurface);
        }
        let (anchor, next_slot, skipped) = match self.next_slot {
            Some(deadline) if now < deadline => {
                return Err(PacingError::TooEarly { deadline });
            }
            Some(deadline) => {
                let skipped = now.saturating_sub(deadline).as_nanos() / self.period.as_nanos();
                let used = checked_add(deadline, checked_multiply(self.period, skipped)?)?;
                (self.anchor, Some(checked_add(used, self.period)?), skipped)
            }
            None => (Some(now), Some(checked_add(now, self.period)?), 0),
        };

        self.anchor = anchor;
        self.next_slot = next_slot;
        self.last_observation = Some(now);
        self.surface = Some(target.surface);
        self.pending = Some(PendingPresentation {
            target,
            mode,
            input_at,
            stage: PresentationStage::Preparing,
        });
        self.stats.submitted = self.stats.submitted.saturating_add(1);
        self.stats.skipped_slots =
            self.stats.skipped_slots.saturating_add(u64::try_from(skipped).unwrap_or(u64::MAX));
        Ok(())
    }

    /// Observe completed preparation and transfer the slot to native-frame custody.
    ///
    /// # Errors
    /// Rejects missing, foreign, duplicate, out-of-order or backward observations.
    pub fn prepared(&mut self, target: FrameStamp, now: Duration) -> Result<(), PacingError> {
        self.validate_time(now)?;
        let pending = self.matches(target)?;
        if pending.stage != PresentationStage::Preparing {
            return Err(PacingError::WrongStage {
                expected: PresentationStage::Preparing,
                actual: pending.stage,
            });
        }
        self.pending =
            Some(PendingPresentation { stage: PresentationStage::AwaitingNative, ..pending });
        self.last_observation = Some(now);
        Ok(())
    }

    /// Observe that native presentation accepted the exact prepared frame and release the slot.
    ///
    /// This observation proves queue acceptance by the platform presentation API. It does not
    /// prove physical scanout, compositor display time or synchronization to a vertical blank.
    ///
    /// # Errors
    /// Rejects missing, foreign, duplicate, out-of-order or backward observations.
    pub fn native_accepted(
        &mut self,
        target: FrameStamp,
        now: Duration,
    ) -> Result<(), PacingError> {
        self.validate_time(now)?;
        let pending = self.matches(target)?;
        if pending.stage != PresentationStage::AwaitingNative {
            return Err(PacingError::WrongStage {
                expected: PresentationStage::AwaitingNative,
                actual: pending.stage,
            });
        }
        let latency = now.checked_sub(pending.input_at).ok_or(PacingError::ClockRegression)?;
        let cadence = self
            .stats
            .last_native_acceptance
            .map(|previous| now.checked_sub(previous).ok_or(PacingError::ClockRegression))
            .transpose()?;

        self.pending = None;
        self.last_observation = Some(now);
        self.stats.last_native_acceptance = Some(now);
        match pending.mode {
            PresentationMode::Canonical => {
                self.stats.canonical_accepted = self.stats.canonical_accepted.saturating_add(1);
            }
            PresentationMode::Projected => {
                self.stats.projected_accepted = self.stats.projected_accepted.saturating_add(1);
            }
        }
        self.stats.input_to_native.observe(latency);
        if let Some(interval) = cadence {
            self.stats.native_acceptance_cadence.observe(interval);
        }
        Ok(())
    }

    /// Release a matching frame after a terminal preparation or native presentation failure.
    /// The cadence remains phase-aligned; a later submission consumes only the newest due slot.
    ///
    /// # Errors
    /// Rejects missing, foreign or backward observations without changing custody.
    pub fn abandoned(&mut self, target: FrameStamp, now: Duration) -> Result<(), PacingError> {
        self.validate_time(now)?;
        self.matches(target)?;
        self.pending = None;
        self.last_observation = Some(now);
        self.stats.abandoned = self.stats.abandoned.saturating_add(1);
        Ok(())
    }

    /// Snapshot bounded aggregate observations.
    pub const fn stats(&self) -> PresentationStats {
        self.stats
    }

    /// End the current measurement window and return its bounded aggregate observations.
    ///
    /// Cadence phase, the next slot, surface binding, monotonic-clock observation and any pending
    /// presentation custody remain unchanged. Native-acceptance interval history starts fresh, so
    /// the next acceptance has no cross-window cadence sample. When called with a pending frame,
    /// that frame retains its original input timestamp and contributes its eventual input latency
    /// and acceptance to the new window.
    pub fn take_stats(&mut self) -> PresentationStats {
        std::mem::take(&mut self.stats)
    }

    fn validate_time(&self, now: Duration) -> Result<(), PacingError> {
        if self.last_observation.is_some_and(|previous| now < previous) {
            Err(PacingError::ClockRegression)
        } else {
            Ok(())
        }
    }

    fn matches(&self, observed: FrameStamp) -> Result<PendingPresentation, PacingError> {
        let pending = self.pending.ok_or(PacingError::NoPendingPresentation)?;
        if pending.target != observed {
            Err(PacingError::ForeignTarget { expected: pending.target, observed })
        } else {
            Ok(pending)
        }
    }
}

fn next_slot_after(
    anchor: Duration,
    period: Duration,
    now: Duration,
) -> Result<Duration, PacingError> {
    let elapsed = now.saturating_sub(anchor).as_nanos();
    let intervals = elapsed / period.as_nanos() + 1;
    checked_add(anchor, checked_multiply(period, intervals)?)
}

fn checked_multiply(period: Duration, multiplier: u128) -> Result<Duration, PacingError> {
    let nanoseconds = period.as_nanos().checked_mul(multiplier).ok_or(PacingError::TimeOverflow)?;
    let seconds = nanoseconds / 1_000_000_000;
    let subsecond = (nanoseconds % 1_000_000_000) as u32;
    let seconds = u64::try_from(seconds).map_err(|_| PacingError::TimeOverflow)?;
    Ok(Duration::new(seconds, subsecond))
}

fn checked_add(left: Duration, right: Duration) -> Result<Duration, PacingError> {
    left.checked_add(right).ok_or(PacingError::TimeOverflow)
}

#[cfg(test)]
#[path = "pacing/tests.rs"]
mod tests;
