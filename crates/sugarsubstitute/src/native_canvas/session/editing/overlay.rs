// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the shown document's presentation overlay and its staged gesture step for frames.
//!
//! The overlay combines the shown selection's traced boundary, animated as marching ants, with
//! the effective tool's feedback. Each different overlay takes a new nonzero generation, which
//! display evidence records, so a changed overlay presents a new frame and an unchanged one
//! reuses the shown frame. Ants step every 100 ms from when they first appeared.
use super::*;
use std::sync::Arc;
use sugarsubstitute_canvas::{MarchingAnts, PresentationOverlay};

/// The overlay frames draw and when its ants first appeared.
#[derive(Default)]
pub(super) struct ShownOverlay {
    overlay: Option<PresentationOverlay>,
    generation: u64,
    next_generation: u64,
    ants_since: Option<Duration>,
}

/// Whether two overlays draw the same marks.
fn same(left: &PresentationOverlay, right: &PresentationOverlay) -> bool {
    let boundaries = match (&left.selection, &right.selection) {
        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
        (None, None) => true,
        _ => false,
    };
    boundaries
        && left.selection_offset == right.selection_offset
        && left.outline == right.outline
        && left.ants == right.ants
        && left.document == right.document
}

impl Session {
    /// The overlay frames of the shown document draw.
    pub(in crate::native_canvas::session) fn shown_overlay(&self) -> Option<&PresentationOverlay> {
        self.editing.overlay.overlay.as_ref()
    }

    /// The generation of [`Self::shown_overlay`], zero when there is none.
    pub(in crate::native_canvas::session) const fn shown_overlay_generation(&self) -> u64 {
        self.editing.overlay.generation
    }

    /// Rebuild the overlay from the shown selection outline, the effective tool's feedback and
    /// the ants' step at `now`, taking a new generation when it changed.
    pub(super) fn refresh_overlay(&mut self, now: Duration) {
        let shown = self.input_shown().then(|| self.document.shown()).flatten();
        let next = shown.and_then(|(_, snapshot)| {
            let boundary = self.editing.outlines.get(snapshot.selection());
            let animated = boundary.as_ref().is_some_and(|boundary| !boundary.is_empty());
            let since = &mut self.editing.overlay.ants_since;
            let elapsed = if animated {
                now.saturating_sub(*since.get_or_insert(now))
            } else {
                *since = None;
                Duration::ZERO
            };
            let tool = self.document.tools.overlay();
            (boundary.is_some() || tool.is_some()).then(|| {
                let ants = MarchingAnts::at(elapsed, false);
                PresentationOverlay::new(snapshot.extent(), boundary, tool, ants)
            })
        });
        let shown = &mut self.editing.overlay;
        let unchanged = match (&next, &shown.overlay) {
            (Some(next), Some(current)) => same(next, current),
            (None, None) => true,
            _ => false,
        };
        if unchanged {
            return;
        }
        shown.generation = if next.is_some() {
            shown.next_generation = shown.next_generation.saturating_add(1);
            shown.next_generation
        } else {
            0
        };
        shown.overlay = next;
    }

    /// When editing next needs the session: the ants' next step while they are shown on a
    /// drawable surface, and every frame during a held editing gesture, so a modifier change
    /// without motion is picked up.
    pub(in crate::native_canvas::session) fn editing_deadline(
        &self,
        now: Duration,
    ) -> Option<Duration> {
        let ants = self
            .editing
            .overlay
            .ants_since
            .filter(|_| self.input_shown() && self.drawable)
            .and_then(|since| {
                MarchingAnts::next_step(now.saturating_sub(since), false)
                    .and_then(|step| since.checked_add(step))
            });
        let tools = &self.document.tools;
        let gesture =
            (self.input_shown() && tools.captured() && tools.effective() != ToolId::PAN_ZOOM)
                .then(|| now.checked_add(tools.navigation().frame_period()))
                .flatten();
        [ants, gesture].into_iter().flatten().min()
    }

    /// Latch the shown tools' latest staged step for frames while no frame work is in flight;
    /// each new step takes a new key, so frame reuse repaints exactly what the step changed.
    pub(super) fn latch_staged(&mut self) {
        if self.rendering.is_some()
            || self.displaying.is_some()
            || self.gpu_presentation_pending()
            || self.awaiting_gpu.is_some()
            || self.document.cpu_pending.is_some()
            || self.document.pending_cpu_evidence.is_some()
            || self.document.pending_gpu_evidence.is_some()
        {
            return;
        }
        let document = &mut *self.document;
        let step = document.tools.preview().map(|preview| preview.session().get());
        let Some((session, snapshot)) = step.zip(document.tools.preview_snapshot()) else {
            document.staged = None;
            return;
        };
        let (base, step) =
            ((snapshot.identity(), snapshot.revision()), (session, snapshot.stage()));
        if document.staged.as_ref().is_some_and(|staged| staged.base == base && staged.step == step)
        {
            return;
        }
        document.staged_generation = document.staged_generation.saturating_add(1);
        let key = (base.0, base.1, document.staged_generation);
        document.staged = Some(document::Staged { base, step, key, snapshot });
    }
}
