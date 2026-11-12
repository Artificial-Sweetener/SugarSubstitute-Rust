// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain prepared storage and drive owned shutdown without waiting on native callbacks.

use crate::{DesktopError, WindowContent};
use std::{num::NonZeroUsize, time::Duration};
use sugarsubstitute_contracts::presentation::{FrameShape, PreparedFrame};

/// Own the event-loop phase and the last successfully published immutable result.
pub(crate) struct SurfaceState {
    pub(crate) closing: bool,
    pub(crate) settled: bool,
    pub(crate) visible: bool,
    pub(crate) failure: Option<DesktopError>,
    pub(crate) retained: Option<PreparedFrame>,
    dirty: bool,
    requested_shape: Option<FrameShape>,
    timings: Option<crate::timing::TimingObserver>,
}

impl SurfaceState {
    /// Start hidden with a pending first preparation once native visibility is established.
    pub(crate) fn new(timings: Option<crate::timing::TimingObserver>) -> Self {
        Self {
            closing: false,
            settled: false,
            visible: false,
            failure: None,
            retained: None,
            dirty: true,
            requested_shape: None,
            timings,
        }
    }

    /// Record input preparation without requesting presentation of the old frame.
    pub(crate) fn dirty(&mut self) {
        if !self.closing {
            self.dirty = true;
        }
    }

    /// Preserve the first cause and enter the same owned close path as user closure.
    pub(crate) fn fail(&mut self, content: &mut impl WindowContent, error: DesktopError) {
        DesktopError::retain_first(&mut self.failure, error);
        self.close(content);
    }

    /// Preserve explicit user intent separately from failure-triggered shutdown.
    pub(crate) fn close_by_user(&mut self, content: &mut impl WindowContent) {
        if !self.closing && self.failure.is_none() {
            content.user_close_requested();
        }
        self.close(content);
    }

    /// Disable publication/input before invalidation and one nonblocking shutdown request.
    pub(crate) fn close(&mut self, content: &mut impl WindowContent) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.visible = false;
        self.dirty = false;
        if let Err(error) = content.set_visible(false) {
            DesktopError::retain_first(
                &mut self.failure,
                DesktopError::boxed("hide_content", error),
            );
        }
        if let Err(error) = content.request_shutdown() {
            DesktopError::retain_first(
                &mut self.failure,
                DesktopError::boxed("request_shutdown", error),
            );
        }
    }

    /// Invalidate changed visibility before any admission; hidden work still gets polled.
    pub(crate) fn visibility(&mut self, content: &mut impl WindowContent, visible: bool) {
        if self.closing || visible == self.visible {
            return;
        }
        match content.set_visible(visible) {
            Ok(()) => {
                self.visible = visible;
                self.dirty |= visible;
            }
            Err(error) => self.fail(content, DesktopError::boxed("set_content_visibility", error)),
        }
    }

    /// Submit only dirty/due visible work, without touching the retained native image.
    pub(crate) fn prepare(
        &mut self,
        content: &mut impl WindowContent,
        shape: Option<FrameShape>,
        elapsed: Duration,
        animation_due: bool,
    ) {
        if self.closing || !self.visible {
            return;
        }
        let Some(shape) = shape else { return };
        if self.requested_shape != Some(shape) {
            self.dirty = true;
        }
        if !self.dirty && !animation_due {
            return;
        }
        if let Some(observer) = &self.timings
            && let Err(cause) = observer.first_preparation()
        {
            self.fail(content, cause);
            return;
        }
        match content.request_frame(shape, elapsed) {
            Ok(()) => {
                self.requested_shape = Some(shape);
                self.dirty = false;
            }
            Err(error) => self.fail(content, DesktopError::boxed("request_frame", error)),
        }
    }

    /// Bounded terminal acknowledgement continues while hidden or closing.
    ///
    /// Only a visible running surface with the actual shape replaces retained pixels.
    /// The content owner has already enforced surface/state/sample publication identity.
    pub(crate) fn poll(
        &mut self,
        content: &mut impl WindowContent,
        actual_shape: Option<FrameShape>,
        budget: NonZeroUsize,
        elapsed: Duration,
    ) -> bool {
        // request_frame owns shape/state invalidation before it admits work. While
        // visible, update changed native geometry before typed completion publication;
        // hidden/closing preparation remains suppressed by prepare's phase guard.
        if self.requested_shape != actual_shape {
            self.prepare(content, actual_shape, elapsed, false);
        }
        match content.poll_frame(budget) {
            Ok(Some(frame))
                if !self.closing && self.visible && Some(frame.shape()) == actual_shape =>
            {
                self.retained = Some(frame);
                true
            }
            Ok(_) => false,
            Err(error) => {
                self.fail(content, DesktopError::boxed("poll_frame", error));
                false
            }
        }
    }

    /// Exit becomes legal only after the owner proves actual settlement.
    pub(crate) fn poll_shutdown(&mut self, content: &mut impl WindowContent) {
        if !self.closing || self.settled {
            return;
        }
        match content.poll_shutdown() {
            Ok(settled) => self.settled = settled,
            Err(error) => {
                DesktopError::retain_first(
                    &mut self.failure,
                    DesktopError::boxed("poll_shutdown", error),
                );
            }
        }
    }

    /// Borrow matching retained pixels for native exposure without admitting preparation.
    pub(crate) fn exposable(&self, shape: Option<FrameShape>) -> Option<&PreparedFrame> {
        if self.closing || !self.visible {
            return None;
        }
        self.retained.as_ref().filter(|frame| Some(frame.shape()) == shape)
    }
}

#[cfg(test)]
#[path = "runtime_state_tests.rs"]
mod tests;
