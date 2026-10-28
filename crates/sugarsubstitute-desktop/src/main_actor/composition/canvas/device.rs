// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind a worker-prepared drawing device to the current composition tree.

use super::super::{adopt, missing};
use crate::{DesktopError, canvas::CanvasGraphics};
use std::sync::Arc;
use windows::{
    Win32::Graphics::DirectComposition::{
        IDCompositionDevice, IDCompositionDevice2, IDCompositionSurfaceFactory,
    },
    core::Interface,
};

/// Retain the prepared device and every composition output through native retirement.
pub(super) struct Device {
    surfaces: Option<IDCompositionSurfaceFactory>,
    composition: Option<IDCompositionDevice2>,
    graphics: Arc<CanvasGraphics>,
}

impl Device {
    /// Retain the worker-prepared device until the composition owner retires.
    pub(super) fn new(graphics: Arc<CanvasGraphics>) -> Self {
        Self { graphics, composition: None, surfaces: None }
    }

    /// Check the actual prepared device identity across frame submissions.
    pub(super) fn owns_graphics(&self, graphics: &Arc<CanvasGraphics>) -> bool {
        Arc::ptr_eq(&self.graphics, graphics)
    }

    /// Bind the completed worker device to this retained composition device.
    pub(super) fn initialize(
        &mut self,
        composition: &IDCompositionDevice,
    ) -> Result<(), DesktopError> {
        let operation = "create_canvas_surface_factory";
        if self.composition.is_none() {
            let mut raw = std::ptr::null_mut();
            // SAFETY: The retained composition device receives the exact requested IID.
            let status = unsafe {
                (composition.vtable().base__.QueryInterface)(
                    composition.as_raw(),
                    &IDCompositionDevice2::IID,
                    &mut raw,
                )
            };
            // SAFETY: The requested interface's transferred output is adopted on all statuses.
            unsafe { adopt(&mut self.composition, raw, status, operation)? };
        }
        if self.surfaces.is_none() {
            let device = self.composition.as_ref().ok_or_else(|| missing("canvas_composition"))?;
            let drawing =
                self.graphics.drawing().ok_or_else(|| missing("canvas_drawing_device"))?;
            let mut raw = std::ptr::null_mut();
            // SAFETY: Both devices remain live through the transferred factory's lifetime.
            let status = unsafe {
                (device.vtable().CreateSurfaceFactory)(device.as_raw(), drawing.as_raw(), &mut raw)
            };
            // SAFETY: CreateSurfaceFactory transfers the requested factory reference.
            unsafe { adopt(&mut self.surfaces, raw, status, operation)? };
        }
        Ok(())
    }

    /// Borrow the surface factory after successful binding.
    pub(super) fn surfaces(&self) -> Result<&IDCompositionSurfaceFactory, DesktopError> {
        self.surfaces.as_ref().ok_or_else(|| missing("canvas_surface_factory"))
    }
}
