// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present CPU pixels into an AppKit surface with an explicit native theme fallback.

use crate::{DesktopError, NativeMaterial};
use std::{num::NonZeroU32, rc::Rc};
use sugarsubstitute_contracts::presentation::PreparedFrame;
use tao::window::Window;

/// Retain the borrowed AppKit window and CPU presentation context in drop order.
pub(crate) struct Presenter {
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    _context: softbuffer::Context<Rc<Window>>,
    background: u8,
}

impl Presenter {
    /// Report the portable native theme background.
    pub(crate) fn material(&self) -> NativeMaterial {
        NativeMaterial::ThemedFallback
    }
    /// Create a real software surface; Mica remains a Windows-owned material.
    pub(crate) fn new(window: Rc<Window>, dark: bool) -> Result<Self, DesktopError> {
        let context = softbuffer::Context::new(Rc::clone(&window))
            .map_err(|error| DesktopError::new("create_appkit_context", error))?;
        let surface = softbuffer::Surface::new(&context, window)
            .map_err(|error| DesktopError::new("create_appkit_surface", error))?;
        Ok(Self { surface, _context: context, background: if dark { 32 } else { 243 } })
    }

    /// Composite premultiplied pixels in CPU memory before submitting the RGB surface.
    pub(crate) fn present(&mut self, frame: &PreparedFrame) -> Result<(), DesktopError> {
        let width = NonZeroU32::new(frame.width())
            .ok_or_else(|| DesktopError::message("resize_appkit_surface", "empty frame width"))?;
        let height = NonZeroU32::new(frame.height())
            .ok_or_else(|| DesktopError::message("resize_appkit_surface", "empty frame height"))?;
        self.surface
            .resize(width, height)
            .map_err(|error| DesktopError::new("resize_appkit_surface", error))?;
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|error| DesktopError::new("borrow_appkit_buffer", error))?;
        for (source, target) in frame.pixels().chunks_exact(4).zip(buffer.iter_mut()) {
            let remainder = (u32::from(self.background) * (255 - u32::from(source[3])) + 127) / 255;
            *target = ((u32::from(source[0]) + remainder) << 16)
                | ((u32::from(source[1]) + remainder) << 8)
                | (u32::from(source[2]) + remainder);
        }
        buffer.present().map_err(|error| DesktopError::new("present_appkit_frame", error))
    }
}
