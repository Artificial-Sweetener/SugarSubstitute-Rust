// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report what embedded canvas presentation costs, one relayed record per second of activity.

use crate::canvas_present_diagnostic::Record;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(1);

/// How one frame's pixels reached Direct2D.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Source {
    /// A new bitmap was allocated and filled from CPU pixels.
    Created,
    /// The surface's retained bitmap was refilled from CPU pixels.
    Updated,
    /// A shared texture was wrapped without copying its pixels.
    Wrapped,
}

/// The measured cost of drawing one frame into its composition surface.
#[derive(Clone, Copy, Debug)]
pub(super) struct UploadCost {
    pub(super) source: Source,
    pub(super) source_bytes: u64,
    /// Opening the surface updates.
    pub(super) open: Duration,
    /// Handing the pixels to Direct2D: creating or refilling a bitmap, or wrapping a texture.
    pub(super) fill: Duration,
    /// Clearing, drawing and closing the surface update.
    pub(super) draw: Duration,
    /// How much of the retained surface this frame redrew.
    pub(super) reach: Reach,
}

/// How much of the retained composition surface one frame redrew.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Reach {
    /// The whole surface was cleared and drawn.
    Whole,
    /// Only the frame's changed regions were redrawn, `scrolled` after moving retained pixels.
    Repaired {
        /// Whether retained pixels were moved first.
        scrolled: bool,
        /// Regions redrawn; none when the frame changed no pixel.
        regions: u32,
    },
}

/// What one committed frame cost after its surface update closed.
#[derive(Clone, Copy, Debug)]
pub(super) struct CommitCost {
    /// Queuing a shared texture's read fence and flushing the device; zero for CPU frames.
    pub(super) signal: Duration,
    /// Binding, placing and committing to composition.
    pub(super) commit: Duration,
    /// The frame's viewport in physical pixels.
    pub(super) width: u32,
    pub(super) height: u32,
}

impl CommitCost {
    /// What a frame of `extent` cost after its surface update closed.
    pub(super) fn new(
        signal: Duration,
        commit: Duration,
        extent: sugarsubstitute_image_ops::Extent,
    ) -> Self {
        Self { signal, commit, width: extent.width(), height: extent.height() }
    }
}

#[derive(Default)]
struct Window {
    presents: u32,
    bitmap_creations: u32,
    bitmap_updates: u32,
    wrapper_creations: u32,
    repaired: u32,
    scrolled: u32,
    unchanged: u32,
    regions: u32,
    source_bytes: u64,
    fill: Duration,
    draw: Duration,
    signal: Duration,
    commit: Duration,
    slowest: Duration,
    width: u32,
    height: u32,
}

/// Accumulate presentation costs on the UI thread; emitting never touches native state.
#[derive(Default)]
pub(super) struct PresentTimings {
    opened: Option<Instant>,
    window: Window,
    #[cfg(test)]
    pub(super) latest: Option<UploadCost>,
    #[cfg(test)]
    pub(super) latest_commit: Option<CommitCost>,
}

impl PresentTimings {
    /// Count one committed frame, closing the window once it spans a second.
    pub(super) fn record(&mut self, cost: UploadCost, committed: CommitCost) {
        let now = Instant::now();
        let opened = *self.opened.get_or_insert(now);
        let window = &mut self.window;
        window.presents = window.presents.saturating_add(1);
        let source = match cost.source {
            Source::Created => &mut window.bitmap_creations,
            Source::Updated => &mut window.bitmap_updates,
            Source::Wrapped => &mut window.wrapper_creations,
        };
        *source = source.saturating_add(1);
        if let Reach::Repaired { scrolled, regions } = cost.reach {
            window.repaired = window.repaired.saturating_add(1);
            window.scrolled = window.scrolled.saturating_add(u32::from(scrolled));
            window.unchanged = window.unchanged.saturating_add(u32::from(regions == 0));
            window.regions = window.regions.saturating_add(regions);
        }
        window.source_bytes = window.source_bytes.saturating_add(cost.source_bytes);
        window.fill += cost.open + cost.fill;
        window.draw += cost.draw;
        window.signal += committed.signal;
        window.commit += committed.commit;
        window.slowest = window
            .slowest
            .max(cost.open + cost.fill + cost.draw + committed.signal + committed.commit);
        (window.width, window.height) = (committed.width, committed.height);
        #[cfg(test)]
        {
            self.latest = Some(cost);
            self.latest_commit = Some(committed);
        }
        if now.duration_since(opened) >= WINDOW {
            self.emit(now.duration_since(opened));
        }
    }

    fn emit(&mut self, elapsed: Duration) {
        if let Some(record) = self.close(elapsed) {
            record.emit();
        }
    }

    /// Close the window into its record; an empty window has none.
    fn close(&mut self, elapsed: Duration) -> Option<Record> {
        let window = std::mem::take(&mut self.window);
        self.opened = None;
        if window.presents == 0 {
            return None;
        }
        let micros = |value: Duration| u32::try_from(value.as_micros()).unwrap_or(u32::MAX);
        Some(Record {
            window_us: micros(elapsed),
            presents: window.presents,
            created: window.bitmap_creations,
            updated: window.bitmap_updates,
            wrapped: window.wrapper_creations,
            repaired: window.repaired,
            scrolled: window.scrolled,
            unchanged: window.unchanged,
            regions: window.regions,
            source_bytes: window.source_bytes,
            fill_us: micros(window.fill),
            draw_us: micros(window.draw),
            signal_us: micros(window.signal),
            commit_us: micros(window.commit),
            slowest_us: micros(window.slowest),
            width: window.width,
            height: window.height,
        })
    }
}

impl Drop for PresentTimings {
    fn drop(&mut self) {
        // The last partial window still describes real presentation work.
        if let Some(opened) = self.opened {
            self.emit(opened.elapsed());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(source: Source, reach: Reach) -> UploadCost {
        UploadCost {
            source,
            source_bytes: if source == Source::Wrapped { 0 } else { 1024 },
            open: Duration::from_micros(10),
            fill: Duration::from_micros(20),
            draw: Duration::from_micros(30),
            reach,
        }
    }

    fn committed(signal: u64, width: u32, height: u32) -> CommitCost {
        CommitCost {
            signal: Duration::from_micros(signal),
            commit: Duration::from_micros(100),
            width,
            height,
        }
    }

    #[test]
    fn a_window_counts_how_much_each_frame_redrew_and_what_its_fence_cost() {
        let mut timings = PresentTimings::default();
        timings.record(frame(Source::Wrapped, Reach::Whole), committed(40, 1920, 1080));
        timings.record(
            frame(Source::Updated, Reach::Repaired { scrolled: true, regions: 3 }),
            committed(0, 3840, 2160),
        );
        timings.record(
            frame(Source::Updated, Reach::Repaired { scrolled: false, regions: 0 }),
            committed(0, 3840, 2160),
        );
        let record = timings.close(Duration::from_secs(1)).ok_or("window absent");
        assert_eq!(
            record,
            Ok(Record {
                window_us: 1_000_000,
                presents: 3,
                created: 0,
                updated: 2,
                wrapped: 1,
                repaired: 2,
                scrolled: 1,
                unchanged: 1,
                regions: 3,
                source_bytes: 2048,
                fill_us: 90,
                draw_us: 90,
                signal_us: 40,
                commit_us: 300,
                slowest_us: 200,
                width: 3840,
                height: 2160,
            })
        );
        assert_eq!(timings.close(Duration::from_secs(1)), None, "a closed window starts empty");
    }
}
