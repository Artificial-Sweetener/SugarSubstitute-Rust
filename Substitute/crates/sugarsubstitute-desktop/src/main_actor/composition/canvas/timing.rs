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

#[derive(Default)]
struct Window {
    presents: u64,
    bitmap_creations: u64,
    bitmap_updates: u64,
    wrapper_creations: u64,
    repaired: u64,
    scrolled: u64,
    source_bytes: u64,
    fill: Duration,
    draw: Duration,
    commit: Duration,
    slowest: Duration,
}

/// Accumulate presentation costs on the UI thread; emitting never touches native state.
#[derive(Default)]
pub(super) struct PresentTimings {
    opened: Option<Instant>,
    window: Window,
    #[cfg(test)]
    pub(super) latest: Option<UploadCost>,
    #[cfg(test)]
    pub(super) latest_commit: Option<Duration>,
}

impl PresentTimings {
    /// Count one committed frame, closing the window once it spans a second.
    pub(super) fn record(&mut self, cost: UploadCost, commit: Duration) {
        let now = Instant::now();
        let opened = *self.opened.get_or_insert(now);
        let window = &mut self.window;
        window.presents += 1;
        match cost.source {
            Source::Created => window.bitmap_creations += 1,
            Source::Updated => window.bitmap_updates += 1,
            Source::Wrapped => window.wrapper_creations += 1,
        }
        if let Reach::Repaired { scrolled, .. } = cost.reach {
            window.repaired += 1;
            window.scrolled += u64::from(scrolled);
        }
        window.source_bytes = window.source_bytes.saturating_add(cost.source_bytes);
        window.fill += cost.open + cost.fill;
        window.draw += cost.draw;
        window.commit += commit;
        window.slowest = window.slowest.max(cost.open + cost.fill + cost.draw + commit);
        #[cfg(test)]
        {
            self.latest = Some(cost);
            self.latest_commit = Some(commit);
        }
        if now.duration_since(opened) >= WINDOW {
            self.emit(now.duration_since(opened));
        }
    }

    fn emit(&mut self, elapsed: Duration) {
        let window = std::mem::take(&mut self.window);
        self.opened = None;
        if window.presents == 0 {
            return;
        }
        let micros = |value: Duration| u64::try_from(value.as_micros()).unwrap_or(u64::MAX);
        Record {
            window_us: micros(elapsed),
            presents: window.presents,
            created: window.bitmap_creations,
            updated: window.bitmap_updates,
            wrapped: window.wrapper_creations,
            repaired: window.repaired,
            scrolled: window.scrolled,
            source_bytes: window.source_bytes,
            fill_us: micros(window.fill),
            draw_us: micros(window.draw),
            commit_us: micros(window.commit),
            slowest_us: micros(window.slowest),
        }
        .emit();
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
