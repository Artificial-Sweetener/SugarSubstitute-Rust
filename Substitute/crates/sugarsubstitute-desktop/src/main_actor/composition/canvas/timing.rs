// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report what embedded canvas presentation costs, one record per second of activity.

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
    /// Opening the surface update and handing the pixels to Direct2D.
    pub(super) fill: Duration,
    /// Clearing, drawing and closing the surface update.
    pub(super) draw: Duration,
}

#[derive(Default)]
struct Window {
    presents: u64,
    bitmap_creations: u64,
    bitmap_updates: u64,
    wrapper_creations: u64,
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
        window.source_bytes = window.source_bytes.saturating_add(cost.source_bytes);
        window.fill += cost.fill;
        window.draw += cost.draw;
        window.commit += commit;
        window.slowest = window.slowest.max(cost.fill + cost.draw + commit);
        #[cfg(test)]
        {
            self.latest = Some(cost);
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
        tracing::info!(
            operation = "canvas_embedded_present",
            window_microseconds = elapsed.as_micros(),
            presents = window.presents,
            bitmap_creations = window.bitmap_creations,
            bitmap_updates = window.bitmap_updates,
            wrapper_creations = window.wrapper_creations,
            source_bytes = window.source_bytes,
            fill_microseconds = window.fill.as_micros(),
            draw_microseconds = window.draw.as_micros(),
            commit_microseconds = window.commit.as_micros(),
            slowest_present_microseconds = window.slowest.as_micros(),
            "Embedded canvas presentation window"
        );
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
