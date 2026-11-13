// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe original form documents and their physical clips after accepted native presentation.
use super::*;
use crate::pixel_capture::PixelCapture;

impl Controls {
    /// Preserve original inventory order and the same page clip used by real HWND peers.
    pub(crate) fn capture(
        &self,
        clips: &[Option<RECT>],
        observer: &mut dyn PixelCapture,
    ) -> Result<(), DesktopError> {
        if clips.len() != self.creation.peers.len() {
            return Err(DesktopError::message(
                "native_capture_clips",
                "Original peer inventory differs",
            ));
        }
        for (slot, clip) in self.creation.peers.iter().zip(clips) {
            let peer = &slot.value;
            if peer.snapshot.visible
                && let Some(engine) = &peer.engine
            {
                engine.capture(&mut |bounds, stride, pixels| {
                    observer.overlay(
                        Some(peer.snapshot.id),
                        rect(bounds),
                        clip.map(rect),
                        stride,
                        pixels,
                    )
                })?;
            }
        }
        if let Some(console) = &self.console {
            console.capture(observer)?;
        }
        Ok(())
    }
}
/// Retain physical coordinates exactly, including partially clipped original peers.
fn rect(value: RECT) -> [i32; 4] {
    [value.left, value.top, value.right, value.bottom]
}
