// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Clip original form peers to the same viewport as their prepared page pixels.
use super::*;
use windows::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject, SetWindowRgn};

impl Controls {
    /// Apply parent-coordinate physical clips in stable inventory order. The OS owns successful
    /// region transfers; failed transfers remain retained until checked cleanup succeeds.
    pub(crate) fn clip(&mut self, clips: &[Option<RECT>]) -> Result<(), DesktopError> {
        if clips.len() != self.creation.peers.len() || !self.creation.writable() {
            return Err(DesktopError::message("native_form_clip", "clip inventory differs"));
        }
        self.release_pending_clip()?;
        for (index, clip) in clips.iter().enumerate() {
            let peer = &self.creation.peers[index].value;
            self.original_peer(peer)?;
            if let Some(clip) = clip {
                let rect = peer.rect;
                let left = clip.left.max(rect.left).min(rect.right) - rect.left;
                let top = clip.top.max(rect.top).min(rect.bottom) - rect.top;
                let right = (clip.right.min(rect.right) - rect.left).max(left);
                let bottom = (clip.bottom.min(rect.bottom) - rect.top).max(top);
                // SAFETY: Create an owned bounded region; no native callback receives it yet.
                let region = unsafe { CreateRectRgn(left, top, right, bottom) };
                if region.is_invalid() {
                    return Err(DesktopError::message(
                        "native_form_clip",
                        "region creation failed",
                    ));
                }
                self.pending_clip = Some(region);
            }
            // SAFETY: The peer is original and affine. Windows takes the region only on success.
            if unsafe { SetWindowRgn(peer.hwnd, self.pending_clip, false) } == 0 {
                return Err(DesktopError::message("native_form_clip", "region transfer failed"));
            }
            self.pending_clip = None;
        }
        Ok(())
    }
    /// Retry an untransferred region's deletion without forgetting an outstanding OS obligation.
    pub(super) fn release_pending_clip(&mut self) -> Result<(), DesktopError> {
        if let Some(region) = self.pending_clip {
            // SAFETY: Only failed transfers remain here; Windows never acquired this region.
            unsafe { DeleteObject(region.into()) }
                .ok()
                .map_err(|cause| DesktopError::new("native_form_clip_release", cause))?;
            self.pending_clip = None;
        }
        Ok(())
    }
}
