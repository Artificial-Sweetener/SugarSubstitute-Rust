// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The one small window a headless role may show beside its pump: its waiting notice.
//!
//! The window is the splash's borderless, transparent shape with the splash presenter. It is
//! created hidden when the role first asks, shown once a frame prepared for its exact shape
//! arrives, never takes focus and has no controls. It closes when the role stops asking or
//! begins shutdown, and never reopens in the same pump. A notice failure is logged and ends
//! the notice; it never fails the role.
use crate::{HeadlessLifetime, HeadlessNotice, NoticeSurface, windows::Presenter};
use std::rc::Rc;
use sugarsubstitute_contracts::presentation::FrameShape;
use tao::{
    dpi::LogicalSize,
    event_loop::EventLoopWindowTarget,
    window::{Theme, Window, WindowBuilder, WindowId},
};

/// The notice window and its presenter, once the role has asked for it.
pub(super) struct Notice {
    window: Option<Rc<Window>>,
    presenter: Option<Presenter>,
    shape: Option<FrameShape>,
    shown: bool,
    /// The notice was opened and closed, or failed; it never reopens.
    retired: bool,
}

impl Notice {
    pub(super) const fn new() -> Self {
        Self { window: None, presenter: None, shape: None, shown: false, retired: false }
    }

    /// Open, update or close the notice for this turn. `closing` closes it at once.
    pub(super) fn turn<T: 'static>(
        &mut self,
        lifetime: &mut impl HeadlessLifetime,
        target: &EventLoopWindowTarget<T>,
        closing: bool,
    ) {
        let wanted = if closing || self.retired { None } else { lifetime.notice() };
        let Some(notice) = wanted else {
            self.close();
            return;
        };
        if let Err(cause) = self.show(notice, lifetime, target) {
            tracing::warn!(operation = "headless_notice", cause = %cause,
                "The starting notice could not be shown; startup continues without it");
            self.close();
            self.retired = true;
        }
    }

    /// Repaint the shown notice's retained pixels when Windows asks.
    pub(super) fn redraw(&self, id: WindowId) {
        let (Some(window), Some(presenter)) = (&self.window, &self.presenter) else { return };
        if self.shown
            && window.id() == id
            && let Err(cause) = presenter.expose_prepared()
        {
            tracing::warn!(operation = "headless_notice_redraw", cause = %cause,
                "The starting notice could not be repainted");
        }
    }

    /// Hide and destroy the window, releasing the presenter's native storage first.
    pub(super) fn close(&mut self) {
        if let Some(window) = &self.window {
            window.set_visible(false);
            self.retired = true;
        }
        if let Some(mut presenter) = self.presenter.take()
            && let Err(cause) = presenter.shutdown()
        {
            tracing::warn!(operation = "headless_notice_close", cause = %cause,
                "The starting notice's pixels could not be released");
        }
        self.window = None;
        self.shape = None;
        self.shown = false;
    }

    fn show<T: 'static>(
        &mut self,
        notice: HeadlessNotice,
        lifetime: &mut impl HeadlessLifetime,
        target: &EventLoopWindowTarget<T>,
    ) -> Result<(), crate::DesktopError> {
        if self.window.is_none() {
            self.open(&notice, lifetime, target)?;
        }
        let Some(frame) = notice.frame else { return Ok(()) };
        if Some(frame.shape()) != self.shape {
            return Ok(());
        }
        let window = self.window.as_ref().ok_or_else(|| absent("window"))?;
        let presenter = self.presenter.as_mut().ok_or_else(|| absent("presenter"))?;
        presenter.present(&frame, None)?;
        if !self.shown {
            window.set_visible(true);
            self.shown = true;
        }
        presenter.expose_prepared()
    }

    /// Create the hidden window, centred on the cursor's monitor like the splash, and report
    /// its exact surface so the role prepares frames for it.
    fn open<T: 'static>(
        &mut self,
        notice: &HeadlessNotice,
        lifetime: &mut impl HeadlessLifetime,
        target: &EventLoopWindowTarget<T>,
    ) -> Result<(), crate::DesktopError> {
        let [width, height] = notice.logical_size;
        let window = Rc::new(
            WindowBuilder::new()
                .with_title(notice.title)
                .with_inner_size(LogicalSize::new(width, height))
                .with_decorations(false)
                .with_resizable(false)
                .with_transparent(true)
                .with_theme(None)
                .with_focused(false)
                .with_visible(false)
                .build(target)
                .map_err(|cause| crate::DesktopError::new("create_headless_notice", cause))?,
        );
        let dark = window.theme() == Theme::Dark;
        let presenter = Presenter::new(window.clone(), dark)?;
        let size = window.inner_size();
        let shape = FrameShape::new(size.width, size.height, window.scale_factor())
            .map_err(|cause| crate::DesktopError::new("headless_notice_shape", cause))?;
        self.window = Some(window);
        self.presenter = Some(presenter);
        self.shape = Some(shape);
        lifetime.notice_surface(NoticeSurface {
            shape,
            dark,
            accent: crate::windows_system_accent::current(),
        });
        Ok(())
    }
}

fn absent(part: &'static str) -> crate::DesktopError {
    crate::DesktopError::message("headless_notice", format!("notice {part} absent"))
}
