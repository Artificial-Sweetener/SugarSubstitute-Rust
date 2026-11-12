// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present CPU frames through Tao's GTK container on both X11 and Wayland.

use crate::runtime::callback_failure::CallbackFailure;
use crate::{
    DesktopError, NativeMaterial,
    native_storage::{CopiedImage, NativeBytes},
};
use cairo::{Format, ImageSurface};
use gtk::{DrawingArea, prelude::*};
use std::{cell::RefCell, rc::Rc};
use sugarsubstitute_contracts::presentation::PreparedFrame;
use tao::{platform::unix::WindowExtUnix, window::Window};

/// Retain the native GTK view and its current CPU image until the surface closes.
pub(crate) struct Presenter {
    _window: Rc<Window>,
    area: DrawingArea,
    image: Rc<RefCell<CopiedImage<ImageSurface>>>,
    failure: Rc<RefCell<CallbackFailure>>,
}

impl Presenter {
    /// Report the portable native theme background.
    pub(crate) fn material(&self) -> NativeMaterial {
        NativeMaterial::ThemedFallback
    }
    /// Mount one native GTK drawing area without depending on X11-only raw handles.
    pub(crate) fn new(window: Rc<Window>, dark: bool) -> Result<Self, DesktopError> {
        let area = DrawingArea::new();
        let image = Rc::new(RefCell::new(CopiedImage::<ImageSurface>::default()));
        let failure = Rc::new(RefCell::new(CallbackFailure::default()));
        let draw_image = Rc::clone(&image);
        let draw_failure = Rc::clone(&failure);
        area.connect_draw(move |area, context| {
            let result = (|| {
                let shade = if dark { 0.125 } else { 0.953 };
                context.set_source_rgb(shade, shade, shade);
                context.paint()?;
                let retained = draw_image.borrow();
                if let Some(image) = retained.image() {
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
                draw_failure.borrow_mut().record(DesktopError::new("paint_gtk_frame", error));
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
    pub(crate) fn present(&mut self, frame: &PreparedFrame) -> Result<(), DesktopError> {
        self.check_callbacks()?;
        self.image.borrow_mut().prepare(frame, copy_image)?;
        self.area.queue_draw();
        Ok(())
    }

    /// Observe deferred Cairo failures even when static content never presents another frame.
    pub(crate) fn check_callbacks(&mut self) -> Result<(), DesktopError> {
        self.failure.borrow_mut().take()
    }

    /// Detach the callback-owning view before the final error check and resource drop.
    pub(crate) fn shutdown(&mut self) -> Result<(), DesktopError> {
        self.detach();
        self.check_callbacks()
    }

    /// Detach once; repeating from Drop is harmless after explicit shutdown.
    fn detach(&self) {
        if let Some(parent) =
            self.area.parent().and_then(|parent| parent.downcast::<gtk::Container>().ok())
        {
            parent.remove(&self.area);
        }
    }
}

impl Drop for Presenter {
    /// Detach the GTK child before the window and draw callback captures are released.
    fn drop(&mut self) {
        self.detach();
    }
}

/// Attach the exact admitted byte-copy owner to Cairo until its last native surface drops.
fn copy_image(frame: &PreparedFrame) -> Result<ImageSurface, DesktopError> {
    let width = i32::try_from(frame.width())
        .map_err(|error| DesktopError::new("allocate_cairo_image", error))?;
    let height = i32::try_from(frame.height())
        .map_err(|error| DesktopError::new("allocate_cairo_image", error))?;
    let bytes = NativeBytes::new(frame)?;
    let stride = Format::ARgb32
        .stride_for_width(frame.width())
        .map_err(|error| DesktopError::new("allocate_cairo_image", error))?;
    ImageSurface::create_for_data(bytes, Format::ARgb32, width, height, stride)
        .map_err(|error| DesktopError::new("allocate_cairo_image", error))
}
