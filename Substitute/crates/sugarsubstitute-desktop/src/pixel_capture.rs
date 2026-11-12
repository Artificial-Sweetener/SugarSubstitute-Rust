// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow actual prepared and native overlay pixels for offscreen rendering qualification.
use sugarsubstitute_contracts::presentation::PreparedFrame;

/// Same-thread observer of a complete accepted surface and its native overlays.
/// Callbacks borrow existing admitted storage; implementations retain their own bounded copies.
/// This captures renderer output, excluding DWM material, operating-system carets and popup windows.
pub trait PixelCapture {
    /// Begin with the exact accepted premultiplied RGBA frame.
    fn surface(&mut self, frame: &PreparedFrame);
    /// Consume a native premultiplied BGRA overlay in physical parent coordinates.
    /// `bounds` and `clip` are left/top/right/bottom; `stride` is bytes per source row.
    /// `control` identifies an original text peer, or is absent for the page scrollbar.
    fn overlay(
        &mut self,
        control: Option<u16>,
        bounds: [i32; 4],
        clip: Option<[i32; 4]>,
        stride: usize,
        pixels: &[u8],
    );
    /// Mark completion only after every current native overlay was successfully read.
    fn finished(&mut self);
}
