// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound native input-age and callback-duration observations independently of image frame timing.
use std::time::Duration;

/// Message age in the wrapping Windows message/tick epoch; no pointer coordinates are retained.
#[derive(Clone, Copy, Default)]
pub(super) struct InputDelay {
    pub(super) messages: u64,
    pub(super) maximum_ms: u32,
}
impl InputDelay {
    /// Preserve timestamp wrap while rejecting a timestamp ahead of the sampled tick counter.
    pub(super) fn observe(&mut self, now: u32, sent: i32) {
        let age = now.wrapping_sub(sent as u32);
        if age <= i32::MAX as u32 {
            self.messages = self.messages.saturating_add(1);
            self.maximum_ms = self.maximum_ms.max(age);
        }
    }
    fn merge(&mut self, other: Self) {
        self.messages = self.messages.saturating_add(other.messages);
        self.maximum_ms = self.maximum_ms.max(other.maximum_ms);
    }
}

#[derive(Clone, Copy, Default)]
struct Sample {
    count: u64,
    maximum: Duration,
    total: Duration,
}
impl Sample {
    fn observe(&mut self, duration: Duration) {
        self.count = self.count.saturating_add(1);
        self.maximum = self.maximum.max(duration);
        self.total = self.total.saturating_add(duration);
    }
}

#[derive(Default)]
struct Window {
    elapsed: Duration,
    activity: u64,
    exposures: u64,
    input: Sample,
    other: Sample,
    poll: Sample,
    native: InputDelay,
}

/// One fixed-size diagnostic window per native event loop. This schedules and retains no work.
#[derive(Default)]
pub(super) struct Timing {
    window: Window,
    last_report: Duration,
}
impl Timing {
    /// Count host activity requests separately from OS exposures; both coalesce before drawing.
    pub(super) fn activity(&mut self) {
        self.window.activity = self.window.activity.saturating_add(1);
    }
    /// Count OS restoration events without equating them to newly prepared image content.
    pub(super) fn exposure(&mut self) {
        self.window.exposures = self.window.exposures.saturating_add(1);
    }
    /// Record complete callback time; polling is an inclusive subset of non-input callbacks.
    pub(super) fn turn(&mut self, input: bool, elapsed: Duration) {
        if input { &mut self.window.input } else { &mut self.window.other }.observe(elapsed);
    }

    /// Measure only synchronous application advancement, with no timer, future or worker wait.
    #[cfg(windows)]
    pub(super) fn poll<T>(&mut self, operation: impl FnOnce() -> T) -> T {
        let started = std::time::Instant::now();
        let result = operation();
        self.window.poll.observe(started.elapsed());
        result
    }

    fn take(&mut self, now: Duration, native: InputDelay, force: bool) -> Option<Window> {
        self.window.native.merge(native);
        if !force && now.saturating_sub(self.last_report) < Duration::from_secs(1) {
            return None;
        }
        let mut window = std::mem::take(&mut self.window);
        window.elapsed = now.saturating_sub(self.last_report);
        self.last_report = now;
        Some(window)
    }

    /// Publish at most one aggregate per active second through the existing diagnostic owner.
    /// Message age uses millisecond OS ticks; it is distinct from high-resolution callback time.
    #[cfg(windows)]
    pub(super) fn report(&mut self, now: Duration, native: InputDelay, force: bool) {
        if let Some(window) = self.take(now, native, force) {
            tracing::info!(
                operation = "canvas_native_input_timing",
                window_microseconds = window.elapsed.as_micros(),
                native_activity_events = window.activity,
                native_exposure_events = window.exposures,
                interaction_callbacks = window.input.count,
                max_interaction_microseconds = window.input.maximum.as_micros(),
                total_interaction_microseconds = window.input.total.as_micros(),
                other_callbacks = window.other.count,
                max_other_microseconds = window.other.maximum.as_micros(),
                total_other_microseconds = window.other.total.as_micros(),
                application_polls = window.poll.count,
                max_poll_microseconds = window.poll.maximum.as_micros(),
                native_messages = window.native.messages,
                max_native_message_age_ms = window.native.maximum_ms,
                "Native input and event-loop timing window"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_message_age_preserves_wrap_and_ignores_future_timestamps() {
        let mut delay = InputDelay::default();
        delay.observe(5, -5);
        delay.observe(10, 11);
        delay.observe(90, 20);
        assert_eq!(delay.messages, 2);
        assert_eq!(delay.maximum_ms, 70);
    }

    #[test]
    fn diagnostic_windows_retain_maxima_until_one_bounded_report_then_reset()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut timing = Timing::default();
        timing.activity();
        timing.activity();
        timing.exposure();
        timing.turn(true, Duration::from_micros(17));
        timing.turn(true, Duration::from_micros(4));
        timing.turn(false, Duration::from_millis(30));
        timing.window.poll.observe(Duration::from_millis(29));
        assert!(
            timing
                .take(Duration::from_millis(900), InputDelay { messages: 2, maximum_ms: 42 }, false)
                .is_none()
        );
        let window = timing
            .take(Duration::from_secs(1), InputDelay { messages: 3, maximum_ms: 7 }, false)
            .ok_or("Missing due timing window")?;
        assert_eq!(window.input.count, 2);
        assert_eq!(window.activity, 2);
        assert_eq!(window.exposures, 1);
        assert_eq!(window.input.maximum, Duration::from_micros(17));
        assert_eq!(window.input.total, Duration::from_micros(21));
        assert_eq!(window.elapsed, Duration::from_secs(1));
        assert_eq!(window.other.maximum, Duration::from_millis(30));
        assert_eq!(window.poll.maximum, Duration::from_millis(29));
        assert_eq!(window.native.messages, 5);
        assert_eq!(window.native.maximum_ms, 42);
        let empty = timing
            .take(Duration::from_millis(1001), InputDelay::default(), true)
            .ok_or("Missing forced timing window")?;
        assert_eq!(empty.input.count, 0);
        assert_eq!(empty.activity, 0);
        assert_eq!(empty.exposures, 0);
        assert_eq!(empty.native.messages, 0);
        Ok(())
    }
}
