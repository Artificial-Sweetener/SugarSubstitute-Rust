// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present CPU frames through Tao's GTK container on both X11 and Wayland.

use crate::{DesktopError, Frame, NativeMaterial};
use cairo::{Format, ImageSurface};
use gtk::{DrawingArea, prelude::*};
use std::{cell::RefCell, rc::Rc};
use tao::{platform::unix::WindowExtUnix, window::Window};

/// Retain the native GTK view and its current CPU image until the surface closes.
pub(crate) struct Presenter {
    _window: Rc<Window>,
    area: DrawingArea,
    image: Rc<RefCell<Option<ImageSurface>>>,
    failure: Rc<RefCell<Option<cairo::Error>>>,
}

impl Presenter {
    /// Report the portable native theme background.
    pub(crate) fn material(&self) -> NativeMaterial {
        NativeMaterial::ThemedFallback
    }
    /// Mount one native GTK drawing area without depending on X11-only raw handles.
    pub(crate) fn new(window: Rc<Window>, dark: bool) -> Result<Self, DesktopError> {
        let area = DrawingArea::new();
        let image = Rc::new(RefCell::new(None::<ImageSurface>));
        let failure = Rc::new(RefCell::new(None));
        let draw_image = Rc::clone(&image);
        let draw_failure = Rc::clone(&failure);
        area.connect_draw(move |area, context| {
            let result = (|| {
                let shade = if dark { 0.125 } else { 0.953 };
                context.set_source_rgb(shade, shade, shade);
                context.paint()?;
                if let Some(image) = draw_image.borrow().as_ref() {
                    context.save()?;
                    context.scale(
                        f64::from(area.allocated_width()) / f64::from(image.width()),
                        f64::from(area.allocated_height()) / f64::from(image.height()),
                    );
                    context.set_source_surface(image, 0.0, 0.0)?;
                    let result = context.paint();
                    let restored = context.restore();
                    result?;
                    restored?;
                }
                Ok::<(), cairo::Error>(())
            })();
            if let Err(error) = result {
                *draw_failure.borrow_mut() = Some(error);
            }
            gtk::glib::Propagation::Proceed
        });
        let container = window.default_vbox().ok_or_else(|| {
            DesktopError::message("mount_gtk_presenter", "Tao window has no default GTK container")
        })?;
        container.pack_start(&area, true, true, 0);
        area.show();
        Ok(Self { _window: window, area, image, failure })
    }

    /// Queue the owned native drawing area with an alpha-composited CPU frame.
    pub(crate) fn present(&mut self, frame: &Frame) -> Result<(), DesktopError> {
        if let Some(error) = self.failure.borrow_mut().take() {
            return Err(DesktopError::new("paint_gtk_frame", error));
        }
        let width = i32::try_from(frame.width)
            .map_err(|error| DesktopError::new("allocate_cairo_image", error))?;
        let height = i32::try_from(frame.height)
            .map_err(|error| DesktopError::new("allocate_cairo_image", error))?;
        let mut bytes = Vec::with_capacity(frame.premultiplied_rgba.len());
        for rgba in frame.premultiplied_rgba.chunks_exact(4) {
            let argb = (u32::from(rgba[3]) << 24)
                | (u32::from(rgba[0]) << 16)
                | (u32::from(rgba[1]) << 8)
                | u32::from(rgba[2]);
            bytes.extend_from_slice(&argb.to_ne_bytes());
        }
        let stride = Format::ARgb32
            .stride_for_width(frame.width)
            .map_err(|error| DesktopError::new("allocate_cairo_image", error))?;
        let image = ImageSurface::create_for_data(bytes, Format::ARgb32, width, height, stride)
            .map_err(|error| DesktopError::new("allocate_cairo_image", error))?;
        *self.image.borrow_mut() = Some(image);
        self.area.queue_draw();
        Ok(())
    }
}

impl Drop for Presenter {
    /// Detach the GTK child before the window and draw callback captures are released.
    fn drop(&mut self) {
        if let Some(parent) =
            self.area.parent().and_then(|parent| parent.downcast::<gtk::Container>().ok())
        {
            parent.remove(&self.area);
        }
    }
}
