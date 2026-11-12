// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select native presentation deadlines without introducing a background scheduler.

use std::time::{Duration, Instant};
use tao::event_loop::ControlFlow;

/// Cap animated frames at approximately sixty hertz without adding raster work to the period.
const FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);

/// Poll only the closing native event loop, leaving all execution workers with their owner.
pub(crate) fn closing_wait(now: Instant) -> ControlFlow {
    ControlFlow::WaitUntil(now + Duration::from_millis(16))
}

/// Suspend static or hidden native surfaces while preserving bounded qualification exit.
pub(crate) fn presentation_wait(
    visible: bool,
    minimized: bool,
    next_frame: Option<Instant>,
    shutdown: Option<Instant>,
) -> ControlFlow {
    let frame = next_frame.filter(|_| visible && !minimized);
    match (frame, shutdown) {
        (Some(frame), Some(shutdown)) => ControlFlow::WaitUntil(frame.min(shutdown)),
        (Some(frame), None) => ControlFlow::WaitUntil(frame),
        (None, Some(shutdown)) => ControlFlow::WaitUntil(shutdown),
        (None, None) => ControlFlow::Wait,
    }
}

/// Skip missed periods while preserving cadence; early interaction keeps its next deadline.
pub(crate) fn advance_deadline(deadline: Instant, now: Instant) -> Instant {
    if now < deadline {
        return deadline;
    }
    let periods = now.duration_since(deadline).as_nanos() / FRAME_INTERVAL.as_nanos() + 1;
    u32::try_from(periods)
        .ok()
        .and_then(|periods| FRAME_INTERVAL.checked_mul(periods))
        .and_then(|delay| deadline.checked_add(delay))
        .unwrap_or_else(|| now + FRAME_INTERVAL)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Owned close remains responsive independently of hidden/static preparation cadence.
    #[test]
    fn closing_poll_has_a_bounded_native_deadline() {
        let now = Instant::now();
        assert_eq!(closing_wait(now), ControlFlow::WaitUntil(now + Duration::from_millis(16)));
    }

    /// Preserve early input cadence and skip missed animation periods without catch-up work.
    #[test]
    fn cadence_skips_late_frames_and_keeps_early_input_deadline() {
        let origin = Instant::now();
        let scheduled = origin + FRAME_INTERVAL;
        assert_eq!(advance_deadline(scheduled, origin + Duration::from_millis(4)), scheduled);
        assert_eq!(
            advance_deadline(scheduled, origin + Duration::from_millis(27)),
            origin + FRAME_INTERVAL * 2
        );
        assert_eq!(
            advance_deadline(scheduled, origin + Duration::from_millis(50)),
            origin + FRAME_INTERVAL * 3
        );
        assert!(FRAME_INTERVAL >= Duration::from_secs(1) / 60);
    }

    /// Stop both static and background animation work, retaining the earliest terminal deadline.
    #[test]
    fn static_hidden_and_minimized_surfaces_wait_for_meaningful_events() {
        let now = Instant::now();
        let next = now + FRAME_INTERVAL;
        let shutdown = now + Duration::from_secs(3);
        for (visible, minimized, frame) in
            [(false, false, Some(next)), (true, true, Some(next)), (true, false, None)]
        {
            assert_eq!(
                presentation_wait(visible, minimized, frame, Some(shutdown)),
                ControlFlow::WaitUntil(shutdown)
            );
            assert_eq!(presentation_wait(visible, minimized, frame, None), ControlFlow::Wait);
        }
        assert_eq!(
            presentation_wait(true, false, Some(next), Some(shutdown)),
            ControlFlow::WaitUntil(next)
        );
        assert_eq!(
            presentation_wait(true, false, Some(next), Some(now)),
            ControlFlow::WaitUntil(now)
        );
    }
}
