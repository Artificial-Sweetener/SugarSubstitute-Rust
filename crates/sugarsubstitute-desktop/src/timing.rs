// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Collect bounded raw native evidence through one thread-affine counter boundary.

use crate::DesktopError;
use sugarsubstitute_contracts::presentation::FrameIdentity;

#[path = "timing_contract.rs"]
mod contract;
pub use contract::*;

#[path = "timing_startup.rs"]
mod startup;
pub(crate) use startup::observe_startup;
pub use startup::{NativeStartupOrder, NativeStartupStage, NativeStartupTiming};

pub(crate) const SAMPLE_LIMIT: usize = 16_384;
pub(crate) const QPC_SOURCE: u64 = 1;

/// Bind one integer tick to its producing native thread and counter source.
#[derive(Clone, Copy)]
pub(crate) struct Stamp {
    pub(crate) ticks: i64,
    pub(crate) thread: u32,
    pub(crate) source: u64,
}

/// Supply a nonblocking raw counter without granting scheduling or native-window access.
pub(crate) trait TimingClock {
    /// Return fixed ticks per second; signed values are validated at admission.
    #[cfg(any(target_os = "windows", test))]
    fn frequency(&self) -> i64;
    /// Return the positive native UI identity captured at construction.
    #[cfg(any(target_os = "windows", test))]
    fn thread(&self) -> u32;
    /// Observe one tick without retaining window handles or scheduling work.
    fn read(&mut self) -> Result<Stamp, DesktopError>;
}

/// Keep bounded receipt storage and the latest validated success identity.
pub(crate) struct TimingAudit {
    clock: Box<dyn TimingClock>,
    receipt: NativeTimingReceipt,
    last_tick: i64,
    last_identity: Option<FrameIdentity>,
}

/// Describe a violated timing boundary without exposing pointer or path data.
fn error(message: &'static str) -> DesktopError {
    DesktopError::message("native_timing", message)
}

impl TimingAudit {
    /// Reserve both buffers before native callback admission; failures retain allocator causes.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn new(
        mut clock: Box<dyn TimingClock>,
        startup_order: NativeStartupOrder,
    ) -> Result<Self, DesktopError> {
        let frequency =
            u64::try_from(clock.frequency()).map_err(|_| error("counter frequency is negative"))?;
        let thread = clock.thread();
        if frequency == 0 || thread == 0 {
            return Err(error("counter frequency and UI thread must be positive"));
        }
        let begin = clock.read()?;
        Self::validate(begin, thread)?;
        let mut callbacks = Vec::new();
        callbacks
            .try_reserve_exact(SAMPLE_LIMIT)
            .map_err(|cause| DesktopError::new("allocate_callback_timings", cause))?;
        let mut submissions = Vec::new();
        submissions
            .try_reserve_exact(SAMPLE_LIMIT)
            .map_err(|cause| DesktopError::new("allocate_submission_timings", cause))?;
        Ok(Self {
            clock,
            last_tick: begin.ticks,
            last_identity: None,
            receipt: NativeTimingReceipt {
                source: NativeTimingSource::WindowsQpc,
                startup_order,
                begin_ticks: begin.ticks,
                end_ticks: begin.ticks,
                frequency,
                ui_thread_id: thread,
                callbacks,
                submissions,
                startup_milestones: [None; 7],
                callbacks_observed: 0,
                callback_samples_omitted: 0,
                submissions_observed: 0,
                submission_samples_omitted: 0,
            },
        })
    }

    /// Reject mismatched source/thread identities and invalid signed counter values.
    fn validate(stamp: Stamp, thread: u32) -> Result<(), DesktopError> {
        if stamp.source != QPC_SOURCE {
            return Err(error("observation has another clock source"));
        }
        if stamp.thread != thread {
            return Err(error("observation is not on the creating UI thread"));
        }
        if stamp.ticks < 0 {
            return Err(error("native counter value is negative"));
        }
        Ok(())
    }

    /// Validate affinity, source and monotonic order before advancing the observed counter.
    pub(crate) fn mark(&mut self) -> Result<Stamp, DesktopError> {
        let stamp = self.clock.read()?;
        Self::validate(stamp, self.receipt.ui_thread_id)?;
        if stamp.ticks < self.last_tick {
            return Err(error("native counter regressed"));
        }
        self.last_tick = stamp.ticks;
        Ok(stamp)
    }

    /// Restrict segment endpoints to the already validated receipt lifetime.
    fn interval(&self, begin: Stamp, end: Stamp) -> Result<NativeTickInterval, DesktopError> {
        Self::validate(begin, self.receipt.ui_thread_id)?;
        Self::validate(end, self.receipt.ui_thread_id)?;
        if begin.ticks < self.receipt.begin_ticks
            || end.ticks > self.last_tick
            || end.ticks < begin.ticks
        {
            return Err(error("segment lies outside validated clock observations"));
        }
        Ok(NativeTickInterval { begin_ticks: begin.ticks, end_ticks: end.ticks })
    }

    /// Account full callbacks while subtracting only the precisely delegated OS drag segment.
    pub(crate) fn callback(
        &mut self,
        begin: Stamp,
        kind: NativeCallbackKind,
        delegated_drag: Option<NativeTickInterval>,
    ) -> Result<(), DesktopError> {
        let end = self.mark()?;
        let interval = self.interval(begin, end)?;
        let full = u64::try_from(interval.end_ticks - interval.begin_ticks)
            .map_err(|_| error("callback ticks overflow"))?;
        let delegated = match delegated_drag {
            Some(segment)
                if segment.begin_ticks >= begin.ticks
                    && segment.end_ticks >= segment.begin_ticks
                    && segment.end_ticks <= end.ticks =>
            {
                u64::try_from(segment.end_ticks - segment.begin_ticks)
                    .map_err(|_| error("delegated ticks overflow"))?
            }
            Some(_) => return Err(error("delegated drag lies outside this callback")),
            None => 0,
        };
        let authored_ticks = full
            .checked_sub(delegated)
            .ok_or_else(|| error("delegated duration exceeds callback"))?;
        let sequence = self
            .receipt
            .callbacks_observed
            .checked_add(1)
            .ok_or_else(|| error("callback sequence overflow"))?;
        let omitted = if self.receipt.callbacks.len() == SAMPLE_LIMIT {
            self.receipt
                .callback_samples_omitted
                .checked_add(1)
                .ok_or_else(|| error("callback omission counter overflow"))?
        } else {
            self.receipt.callback_samples_omitted
        };
        if self.receipt.callbacks.len() < SAMPLE_LIMIT {
            self.receipt.callbacks.push(NativeCallbackTiming {
                sequence,
                kind,
                begin_ticks: begin.ticks,
                end_ticks: end.ticks,
                delegated_drag,
                authored_ticks,
            });
        }
        self.receipt.callbacks_observed = sequence;
        self.receipt.callback_samples_omitted = omitted;
        Ok(())
    }

    /// Record only actual successful submissions; retained identity compares previous success.
    pub(crate) fn submission(
        &mut self,
        begin: Stamp,
        identity: FrameIdentity,
    ) -> Result<(), DesktopError> {
        if self.receipt.startup_milestones[6].is_none_or(|sample| sample.ticks > begin.ticks) {
            return Err(error("successful submission precedes the first preparation request"));
        }
        let end = self.mark()?;
        let interval = self.interval(begin, end)?;
        let sequence = self
            .receipt
            .submissions_observed
            .checked_add(1)
            .ok_or_else(|| error("submission sequence overflow"))?;
        let omitted = if self.receipt.submissions.len() == SAMPLE_LIMIT {
            self.receipt
                .submission_samples_omitted
                .checked_add(1)
                .ok_or_else(|| error("submission omission counter overflow"))?
        } else {
            self.receipt.submission_samples_omitted
        };
        if self.receipt.submissions.len() < SAMPLE_LIMIT {
            self.receipt.submissions.push(NativeSubmissionTiming {
                sequence,
                identity,
                retained: self.last_identity == Some(identity),
                begin_ticks: interval.begin_ticks,
                end_ticks: interval.end_ticks,
            });
        }
        self.last_identity = Some(identity);
        self.receipt.submissions_observed = sequence;
        self.receipt.submission_samples_omitted = omitted;
        Ok(())
    }

    /// Transfer the bounded raw receipt after all native and content teardown returns.
    pub(crate) fn finish(mut self) -> Result<NativeTimingReceipt, DesktopError> {
        self.receipt.end_ticks = self.mark()?.ticks;
        Ok(self.receipt)
    }
}

#[path = "timing_observer.rs"]
mod observer;
pub(crate) use observer::{TimingObserver, observe_callback, observe_submission};

#[cfg(test)]
#[path = "timing_tests.rs"]
mod tests;
