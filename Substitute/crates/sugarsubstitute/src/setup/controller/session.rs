// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose native navigation and rendering beneath the installer's existing execution lifetime.
use super::*;
mod input;
use crate::setup::presentation::{Prepared, View, driver::Driver};
use input::PendingPicker;
use std::time::Duration;
use sugarsubstitute_contracts::{execution::SurfaceId, presentation::FrameShape};

/// Native page state and its prepared pixels. Backend custody and the execution owner stay with
/// Controller; this session owns one child render scope and never drains a second mailbox.
pub(super) struct Session {
    pub(super) navigation: navigation::Navigation,
    view: View,
    driver: Option<Driver>,
    prepared: Option<Prepared>,
    current: bool,
    waiting: bool,
    page_revision: u64,
    picker: Option<PendingPicker>,
    picker_queued: bool,
    next_picker: u64,
    native_close: bool,
}
impl Session {
    /// Retain inert navigation and validated geometry before any execution or native construction.
    pub(super) fn new(install_root: String, view: View) -> Result<Self> {
        FrameShape::new(view.surface.width, view.surface.height, view.surface.scale.into())?;
        Ok(Self {
            navigation: navigation::Navigation::new(install_root)?,
            view,
            driver: None,
            prepared: None,
            current: false,
            waiting: false,
            page_revision: 0,
            picker: None,
            picker_queued: false,
            next_picker: 1,
            native_close: false,
        })
    }
    /// Join the actual installer execution tree with a dedicated renderer. Its scope is
    /// numbered by creation order with every other installer scope; surface 1 belongs to the
    /// native transcript.
    pub(super) fn start(&mut self, controller: &Controller) -> Result<()> {
        if self.started() || self.navigation.flow.is_closing() || controller.closing {
            return Err("Native installer session already started or closing".into());
        }
        let shape = FrameShape::new(
            self.view.surface.width,
            self.view.surface.height,
            self.view.surface.scale.into(),
        )?;
        let client = controller.client.as_ref().ok_or("Installer execution absent")?;
        let parent = controller.scope.as_ref().ok_or("Installer root scope absent")?;
        let scope = controller.create_scope(Some(parent))?;
        self.driver = Some(Driver::new(
            client.clone(),
            scope,
            controller.jobs.clone(),
            SurfaceId::new(2)?,
            shape,
        )?);
        Ok(())
    }
    /// Borrow only a page matching current application state; native fields retain edits independently.
    pub(super) fn page(&self) -> Option<sugarsubstitute_desktop::installer::native::Page<'_>> {
        if !self.ready() {
            return None;
        }
        let prepared = self.prepared.as_ref()?;
        let layout = &prepared.layout;
        Some(sugarsubstitute_desktop::installer::native::Page {
            frame: &prepared.frame,
            shell: &layout.shell_controls,
            controls: &layout.page_controls,
            drag_bounds: layout.drag_bounds,
            viewport: layout.viewport,
            scroll: layout.scroll,
            content_height: layout.content_height,
        })
    }
    /// Invalidate before the adapter can accept a result for a previous client geometry.
    pub(super) fn resize(&mut self, shape: FrameShape) -> Result<()> {
        if self.view.surface.width != shape.width()
            || self.view.surface.height != shape.height()
            || f64::from(self.view.surface.scale) != shape.scale()
        {
            self.invalidate()?;
            self.view.surface.width = shape.width();
            self.view.surface.height = shape.height();
            self.view.surface.scale = shape.scale() as f32;
        }
        Ok(())
    }
    /// Application and native close intent share the same eventual checked retirement.
    pub(super) fn close_requested(&self) -> bool {
        self.native_close || self.navigation.flow.close_requested()
    }
    /// Derive transcript placement from the accepted page and hide stale geometry while preparing.
    pub(super) fn console_layout(
        &self,
    ) -> sugarsubstitute_desktop::installer::InstallerConsoleLayout {
        let s = self.view.surface;
        let bounds = self.prepared.as_ref().and_then(|prepared| {
            prepared.layout.console.map(|b| {
                let viewport = prepared.layout.viewport;
                let x = (viewport.x + b.x).max(viewport.x);
                let y = (viewport.y + b.y).max(viewport.y);
                sugarsubstitute_desktop::ControlBounds {
                    x: x.into(),
                    y: y.into(),
                    width: (b.width.min(viewport.x + viewport.width - x)).max(0.0).into(),
                    height: (b.height.min(viewport.y + viewport.height - y)).max(0.0).into(),
                }
            })
        });
        sugarsubstitute_desktop::installer::InstallerConsoleLayout {
            width: s.width,
            height: s.height,
            scale: s.scale.into(),
            visible: bounds.is_some()
                && !self.navigation.flow.is_closing()
                && self.page_revision == self.navigation.flow.page_revision()
                && self.navigation.flow.details_open()
                && self.prepared.as_ref().is_some_and(|p| {
                    p.frame.width() == s.width
                        && p.frame.height() == s.height
                        && p.frame.scale() == f64::from(s.scale)
                }),
            bounds: bounds.unwrap_or(sugarsubstitute_desktop::ControlBounds {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }),
        }
    }
    /// Actual renderer ownership, distinct from the existence of any prepared page.
    pub(super) fn started(&self) -> bool {
        self.driver.is_some()
    }
    /// Observe that the current application state has a matching accepted native frame.
    pub(super) fn ready(&self) -> bool {
        self.current && self.prepared.is_some()
    }
    /// Invalidate before mutating application input so a completed old paint cannot overwrite it.
    pub(super) fn invalidate(&mut self) -> Result<()> {
        if let Some(driver) = &mut self.driver {
            driver.invalidate()?;
        }
        self.current = false;
        Ok(())
    }
    /// Route the same root completion batch to provisioning and rendering. Apply backend facts and
    /// wait-feedback transitions before admitting any old render; diagnostics retain console custody.
    pub(super) fn advance(&mut self, controller: &mut Controller, now: Duration) -> Result<()> {
        let advanced = controller.advance_events()?;
        if !self.navigation.flow.is_closing()
            && advanced.events.iter().any(|event| !matches!(event.kind, Kind::Diagnostic(_)))
        {
            self.invalidate()?;
        }
        self.navigation.advance(controller, advanced.events, now)?;
        if self.navigation.flow.page_revision() != self.page_revision {
            self.invalidate()?;
            self.page_revision = self.navigation.flow.page_revision();
            self.view.scroll = 0.0;
            self.view.interaction = Default::default();
        }
        let waiting = self.navigation.flow.waiting(now);
        if waiting != self.waiting {
            self.invalidate()?;
            self.waiting = waiting;
        }
        self.view.now = now;
        if let Some(driver) = &mut self.driver
            && let Some(prepared) =
                driver.advance(&self.navigation.flow, self.view, &advanced.completions)?
        {
            self.view.scroll = prepared.layout.scroll;
            self.prepared = Some(prepared);
            self.current = true;
        }
        Ok(())
    }
    /// Seal navigation and rendering before the backend starts process/storage shutdown.
    pub(super) fn close(&mut self) -> Result<()> {
        self.navigation.flow.closing();
        self.picker = None;
        self.picker_queued = false;
        self.current = false;
        if let Some(driver) = &mut self.driver {
            driver.close()?;
        }
        Ok(())
    }
    /// Rendering must actually settle before the root can begin execution shutdown.
    pub(super) fn settled(&self) -> bool {
        self.navigation.flow.is_closing() && self.driver.as_ref().is_none_or(Driver::settled)
    }
    /// Release retained pixels/fonts after native teardown and observed child settlement.
    pub(super) fn finish(&mut self) -> Result<()> {
        if !self.settled() {
            return Err("Native installer session has not settled".into());
        }
        if let Some(driver) = &mut self.driver {
            driver.finish()?;
        }
        self.prepared = None;
        self.driver = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
