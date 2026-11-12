// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show the caption's update button state and its text peers' words on the retained Main peers.
use super::*;
use crate::main_updates::{DISMISS, LINE, LINK, OFFER, UPDATE, UpdateCaptionView, UpdateGlyph};
use windows::Win32::{
    Graphics::Gdi::InvalidateRect,
    UI::WindowsAndMessaging::{GetWindowTextW, IsWindow},
};

impl Controls {
    /// Lay the Main caption out as `controls` and show `view`'s words and update tint.
    /// # Errors
    /// Preserves the first native update, label or repaint failure.
    pub(crate) fn present_caption(
        &mut self,
        controls: &[AccessibleControl],
        view: &UpdateCaptionView,
    ) -> Result<(), DesktopError> {
        self.update(&Specification::buttons(controls))?;
        self.set_update_glyph(view.glyph)?;
        for id in [OFFER, LINE, LINK, DISMISS] {
            let peer = view.peer(id).ok_or_else(|| {
                DesktopError::message("present_caption_text", "caption peer missing")
            })?;
            self.set_caption_text(id, &peer.text)?;
        }
        Ok(())
    }

    /// Tint the update button for `glyph`, repainting it only when the state changes.
    /// # Errors
    /// Rejects a surface that is not the Main caption or whose peer has retired.
    fn set_update_glyph(&self, glyph: UpdateGlyph) -> Result<(), DesktopError> {
        let mut caption =
            self.callback.caption.get().ok_or_else(|| {
                DesktopError::message("present_update_glyph", "caption unconfigured")
            })?;
        if caption.update == glyph {
            return Ok(());
        }
        caption.update = glyph;
        self.callback.caption.set(Some(caption));
        let peer = self.caption_peer(UPDATE)?;
        // SAFETY: The retained peer belongs to this UI thread; invalidation only queues a paint.
        if !unsafe { InvalidateRect(Some(peer.hwnd), None, false) }.as_bool() {
            return Err(DesktopError::message("present_update_glyph", "InvalidateRect failed"));
        }
        Ok(())
    }

    /// Make caption peer `id` show and announce `text`, leaving it untouched when it already
    /// does. Owner-drawn buttons repaint when their text changes.
    /// # Errors
    /// Rejects an unknown or retired peer and words longer than a native label allows.
    fn set_caption_text(&self, id: u16, text: &str) -> Result<(), DesktopError> {
        let peer = self.caption_peer(id)?;
        let mut units = [0_u16; 129];
        let mut length = 0;
        for unit in text.encode_utf16() {
            if length >= units.len() - 1 {
                return Err(DesktopError::message("label_caption_peer", "label bound exceeded"));
            }
            units[length] = unit;
            length += 1;
        }
        let mut current = [0_u16; 129];
        // SAFETY: Read the retained peer's text into a bounded stack buffer.
        let shown = unsafe { GetWindowTextW(peer.hwnd, &mut current) }.max(0) as usize;
        if current[..shown] == units[..length] {
            return Ok(());
        }
        // SAFETY: Exact retained peer; Win32 copies this bounded NUL-terminated stack label.
        unsafe { SetWindowTextW(peer.hwnd, PCWSTR(units.as_ptr())) }
            .map_err(|cause| DesktopError::new("label_caption_peer", cause))
    }

    /// The live Main caption peer with identity `id`.
    fn caption_peer(&self, id: u16) -> Result<&Peer, DesktopError> {
        if !matches!(self.paint_mode, PaintMode::NativeButtons(_)) || !self.creation.writable() {
            return Err(DesktopError::message("caption_peer", "Main caption is not writable"));
        }
        let peer = self
            .creation
            .peers
            .iter()
            .map(|slot| &slot.value)
            .find(|peer| peer.snapshot.id == id)
            .ok_or_else(|| DesktopError::message("caption_peer", "caption peer missing"))?;
        // SAFETY: This creating thread retains the exact registration through the query.
        if !unsafe { IsWindow(Some(peer.hwnd)) }.as_bool() {
            return Err(DesktopError::message("caption_peer", "caption peer retired"));
        }
        Ok(peer)
    }
}

#[cfg(test)]
#[path = "windows_controls_caption_tests.rs"]
#[allow(unsafe_code)]
mod tests;
