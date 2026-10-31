// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present prepared canvas pixels below the browser in its owned composition tree.

mod allocation;
mod device;

use super::{adopt, missing};
use crate::{
    DesktopError,
    canvas::{
        CpuFrame,
        acknowledgement::{Acknowledgement, FrameEffect, HideEffect},
        placement::CanvasPlacement,
    },
};
use allocation::Allocation;
use device::Device;
use std::sync::Arc;
use sugarsubstitute_canvas::FrameStamp;
use sugarsubstitute_contracts::execution::MemoryLease;
use windows::{
    Win32::Graphics::{
        Direct2D::Common::D2D_RECT_F,
        DirectComposition::{IDCompositionDevice, IDCompositionVisual},
    },
    core::Interface,
};
use wry::WebViewExtWindows;

impl super::Composition {
    /// Present only through this retained tree; failed effects remain fenced for retirement.
    fn present_canvas(
        &mut self,
        frame: &crate::canvas::MainCanvasFrame<'_>,
    ) -> Result<Option<FrameStamp>, DesktopError> {
        if self.canvas.is_none() {
            self.canvas = Some(Canvas::new(
                self.device()?.clone(),
                self.root()?.clone(),
                frame.memory.clone(),
                frame.graphics.clone(),
            )?);
        }
        let canvas = self.canvas.as_mut().ok_or_else(|| missing("canvas_composition"))?;
        if !canvas.owns_memory(frame.memory) {
            return Err(DesktopError::message("canvas_composition", "canvas memory owner changed"));
        }
        if !canvas.owns_graphics(frame.graphics) {
            return Err(DesktopError::message(
                "canvas_composition",
                "canvas graphics owner changed",
            ));
        }
        self.ledger.dirty = true;
        let result = canvas.present(&frame.frame, frame.placement)?;
        self.ledger.dirty = false;
        self.ledger.committed = true;
        Ok(result)
    }

    /// Hide a previously committed native canvas before its geometry becomes unusable.
    fn hide_canvas(&mut self) -> Result<bool, DesktopError> {
        if let Some(canvas) = &mut self.canvas {
            self.ledger.dirty = true;
            let hidden = canvas.hide()?;
            self.ledger.dirty = false;
            self.ledger.committed = true;
            return Ok(hidden);
        }
        Ok(true)
    }
}

impl super::MainActor {
    /// Observe the actual WebView allocation and zoom on its creating apartment.
    pub(crate) fn canvas_allocation(
        &self,
    ) -> Result<crate::canvas::placement::WebViewAllocation, DesktopError> {
        let window = self.window()?;
        let size = window.inner_size();
        let mut zoom = 0.0;
        // SAFETY: The retained controller and writable scalar belong to this UI apartment.
        unsafe { self.view()?.controller().ZoomFactor(&mut zoom) }
            .map_err(|cause| DesktopError::new("canvas_webview_zoom", cause))?;
        Ok(crate::canvas::placement::WebViewAllocation {
            bounds: crate::canvas::placement::PhysicalRect {
                x: 0,
                y: 0,
                width: size.width,
                height: size.height,
            },
            physical_pixels_per_css_pixel: window.scale_factor() * zoom,
        })
    }

    /// Accept pixels only for this exact revealed browser lifetime.
    pub(crate) fn present_canvas(
        &mut self,
        frame: &crate::canvas::MainCanvasFrame<'_>,
    ) -> Result<Option<FrameStamp>, DesktopError> {
        if self.identity() != frame.identity || !self.revealed || self.admission.closing() {
            return Ok(None);
        }
        if self.canvas_allocation()? != frame.allocation {
            self.hide_canvas()?;
            return Ok(None);
        }
        let size = self.window()?.inner_size();
        let clip = frame.placement.clip;
        if clip.x < 0
            || clip.y < 0
            || i64::from(clip.x) + i64::from(clip.width) > i64::from(size.width)
            || i64::from(clip.y) + i64::from(clip.height) > i64::from(size.height)
        {
            self.hide_canvas()?;
            return Ok(None);
        }
        self.composition
            .as_mut()
            .ok_or_else(|| missing("browser_composition"))?
            .present_canvas(frame)
    }

    /// Fence previous pixels when no current application frame owns the viewport.
    pub(crate) fn hide_canvas(&mut self) -> Result<bool, DesktopError> {
        if !self.admission.closing()
            && let Some(composition) = &mut self.composition
        {
            return composition.hide_canvas();
        }
        Ok(false)
    }

    /// Keep acknowledged pixels only while their native lifetime and geometry remain exact.
    /// No upload, allocation or replacement acknowledgement occurs on this path.
    pub(crate) fn retain_canvas(
        &mut self,
        retained: crate::canvas::MainCanvasRetention,
    ) -> Result<(), DesktopError> {
        let current = self.identity() == retained.identity
            && self.revealed
            && !self.admission.closing()
            && self.canvas_allocation()? == retained.allocation
            && self
                .composition
                .as_ref()
                .and_then(|tree| tree.canvas.as_ref())
                .is_some_and(|canvas| canvas.retains(retained.target, retained.placement));
        if !current {
            self.hide_canvas()?;
        }
        Ok(())
    }
}

/// Own the graphics device, visual and both sides of any uncommitted resize.
pub(super) struct Canvas {
    composition: Option<IDCompositionDevice>,
    root: Option<IDCompositionVisual>,
    visual: Option<IDCompositionVisual>,
    device: Option<Device>,
    active: Option<Allocation>,
    pending: Option<Allocation>,
    retirements: [Option<RetiredAllocation>; 2],
    acknowledgement: Acknowledgement,
    linked: bool,
    retired: bool,
    lease: Option<MemoryLease>,
}

impl Canvas {
    /// Match actual native custody without treating candidate pixels as displayed.
    fn retains(&self, target: FrameStamp, placement: CanvasPlacement) -> bool {
        !self.retired && self.active.is_some() && self.acknowledgement.retains(target, placement)
    }
    /// Check actual reservation custody, not merely an equal byte limit.
    pub(super) fn owns_memory(&self, lease: &MemoryLease) -> bool {
        self.lease.as_ref().is_some_and(|owned| owned.same_reservation(lease))
    }

    fn owns_graphics(&self, graphics: &Arc<crate::canvas::CanvasGraphics>) -> bool {
        self.device.as_ref().is_some_and(|device| device.owns_graphics(graphics))
    }

    /// Retain admitted storage and native parents before performing graphics mutations.
    pub(super) fn new(
        composition: IDCompositionDevice,
        root: IDCompositionVisual,
        lease: MemoryLease,
        graphics: Arc<crate::canvas::CanvasGraphics>,
    ) -> Result<Self, DesktopError> {
        if !graphics.owns_memory(&lease) {
            return Err(DesktopError::message(
                "canvas_device_budget",
                "graphics reservation changed",
            ));
        }
        Ok(Self {
            composition: Some(composition),
            root: Some(root),
            visual: None,
            device: Some(Device::new(graphics)),
            active: None,
            pending: None,
            retirements: [None, None],
            acknowledgement: Acknowledgement::default(),
            linked: false,
            retired: false,
            lease: Some(lease),
        })
    }

    /// Upload and commit one complete frame, returning acknowledgement only after commit.
    /// A failed native mutation retains both old and replacement resources for tree retirement.
    pub(super) fn present(
        &mut self,
        frame: &CpuFrame<'_>,
        placement: CanvasPlacement,
    ) -> Result<Option<FrameStamp>, DesktopError> {
        let extent = frame.validate()?;
        let clip = clip(placement)?;
        if extent.width() != placement.viewport.width
            || extent.height() != placement.viewport.height
        {
            return Err(DesktopError::message(
                "canvas_placement",
                "frame extent differs from viewport",
            ));
        }
        match self.acknowledgement.frame(
            frame.target,
            placement,
            frame.pending,
            self.has_retirements(),
        )? {
            FrameEffect::Committed(acknowledged) => return Ok(acknowledged),
            FrameEffect::AwaitRetirement => return Ok(None),
            FrameEffect::Commit => {}
        }
        let composition = self.composition.as_ref().ok_or_else(|| missing("canvas_composition"))?;
        let device = self.device.as_mut().ok_or_else(|| missing("canvas_device"))?;
        device.initialize(composition)?;
        if self.visual.is_none() {
            let mut raw = std::ptr::null_mut();
            // SAFETY: This retained device transfers a visual on its creating apartment.
            let status =
                unsafe { (composition.vtable().CreateVisual)(composition.as_raw(), &mut raw) };
            // SAFETY: The exact transferred visual is retained even on partial failure.
            unsafe { adopt(&mut self.visual, raw, status, "create_canvas_visual")? };
        }
        let replacement = self.active.as_ref().is_none_or(|active| active.extent() != extent);
        if replacement {
            if self.pending.is_some() {
                return Err(DesktopError::message(
                    "canvas_surface",
                    "replacement already retained",
                ));
            }
            self.pending = Some(Allocation::new(
                extent,
                self.lease.as_ref().ok_or_else(|| missing("canvas_memory"))?,
            )?);
            self.pending
                .as_mut()
                .ok_or_else(|| missing("pending_canvas_surface"))?
                .create(device.surfaces()?)?;
        }
        let allocation = if replacement { &mut self.pending } else { &mut self.active };
        let allocation = allocation.as_mut().ok_or_else(|| missing("canvas_allocation"))?;
        allocation.upload(frame)?;
        let visual = self.visual.as_ref().ok_or_else(|| missing("canvas_visual"))?;
        // SAFETY: All interfaces are live on the UI apartment; the clip is within this viewport.
        unsafe {
            visual
                .SetContent(allocation.surface()?)
                .map_err(|cause| DesktopError::new("bind_canvas_surface", cause))?;
            #[allow(clippy::cast_precision_loss)]
            visual
                .SetOffsetX2(placement.viewport.x as f32)
                .map_err(|cause| DesktopError::new("position_canvas_surface", cause))?;
            #[allow(clippy::cast_precision_loss)]
            visual
                .SetOffsetY2(placement.viewport.y as f32)
                .map_err(|cause| DesktopError::new("position_canvas_surface", cause))?;
            visual
                .SetClip2(&clip)
                .map_err(|cause| DesktopError::new("clip_canvas_surface", cause))?;
            if !self.linked {
                // A null reference with TRUE places this visual behind all existing siblings.
                // A failing AddVisual may still mutate the tree; keep the complete owner alive.
                self.linked = true;
                self.root
                    .as_ref()
                    .ok_or_else(|| missing("canvas_root"))?
                    .AddVisual(visual, true, None::<&IDCompositionVisual>)
                    .map_err(|cause| DesktopError::new("link_canvas_visual", cause))?;
            }
            composition
                .Commit()
                .map_err(|cause| DesktopError::new("commit_canvas_frame", cause))?;
        }
        if replacement {
            let old = self.active.take();
            self.active = self.pending.take();
            if let Some(allocation) = old {
                self.retirements[0] = Some(RetiredAllocation {
                    _allocation: allocation,
                    commit: super::FencedCommit::new(composition.clone()),
                });
            }
        }
        self.acknowledgement.committed(frame.target, placement);
        Ok(Some(frame.target))
    }

    /// Complete outstanding drawing before the parent tree detaches.
    pub(super) fn end_draws(&mut self) -> Result<(), DesktopError> {
        if let Some(active) = &mut self.active {
            active.end_draw()?;
        }
        if let Some(pending) = &mut self.pending {
            pending.end_draw()?;
        }
        Ok(())
    }

    /// Hide the last committed surface; completion is acknowledged only after its fence.
    pub(super) fn hide(&mut self) -> Result<bool, DesktopError> {
        match self.acknowledgement.hide(self.has_retirements()) {
            HideEffect::Hidden => return Ok(true),
            HideEffect::AwaitRetirement => return Ok(false),
            HideEffect::Commit => {}
        }
        let visual = self.visual.as_ref().ok_or_else(|| missing("canvas_visual"))?;
        let composition =
            self.composition.as_ref().ok_or_else(|| missing("canvas_composition"))?.clone();
        if self.pending.is_some() {
            return Err(DesktopError::message(
                "canvas_surface",
                "uncommitted replacement retained",
            ));
        }
        let slot = self.retirements.iter().position(Option::is_none).ok_or_else(|| {
            DesktopError::message("canvas_surface", "retirement capacity exhausted")
        })?;
        // SAFETY: The retained visual is disconnected before this device commits the change.
        unsafe {
            visual
                .SetContent(None::<&windows::core::IUnknown>)
                .map_err(|cause| DesktopError::new("hide_canvas_surface", cause))?;
            composition.Commit().map_err(|cause| DesktopError::new("commit_canvas_hide", cause))?;
        }
        if let Some(allocation) = self.active.take() {
            self.retirements[slot] = Some(RetiredAllocation {
                _allocation: allocation,
                commit: super::FencedCommit::new(composition),
            });
        }
        self.acknowledgement.hide_committed();
        Ok(false)
    }

    /// Retain every displaced surface until its own commit has actually completed.
    pub(super) fn reap_retirements(&mut self) -> Result<(), DesktopError> {
        for slot in &mut self.retirements {
            if let Some(retired) = slot {
                retired.commit.check()?;
                if retired.commit.completed() {
                    *slot = None;
                }
            }
        }
        Ok(())
    }

    /// Keep presentation and final tree teardown bounded while older surfaces remain in custody.
    pub(super) fn has_retirements(&self) -> bool {
        self.retirements.iter().any(Option::is_some)
    }

    /// Borrow the first worker input still owned by this desktop surface.
    pub(super) fn next_wait(&mut self) -> Option<&mut Option<crate::canvas::CompositionWait>> {
        self.retirements
            .iter_mut()
            .flatten()
            .find_map(|retired| retired.commit.wait.is_some().then_some(&mut retired.commit.wait))
    }

    /// Release only after the parent observed detached-tree and displaced-surface completion.
    pub(super) fn release_after_detach(&mut self) -> Result<(), DesktopError> {
        if self.has_retirements() {
            return Err(DesktopError::message(
                "retire_canvas",
                "surface completion remains pending",
            ));
        }
        self.visual = None;
        self.pending = None;
        self.active = None;
        self.device = None;
        self.root = None;
        self.composition = None;
        self.lease = None;
        self.retired = true;
        Ok(())
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        if !self.retired {
            std::mem::forget(self.visual.take());
            std::mem::forget(self.pending.take());
            std::mem::forget(self.active.take());
            for retirement in &mut self.retirements {
                std::mem::forget(retirement.take());
            }
            std::mem::forget(self.device.take());
            std::mem::forget(self.root.take());
            std::mem::forget(self.composition.take());
            std::mem::forget(self.lease.take());
            tracing::error!(
                operation = "canvas_composition_unresolved_drop",
                "Retaining unresolved native canvas resources until process termination"
            );
        }
    }
}

/// Keep the displaced surface and its original memory claim through compositor completion.
struct RetiredAllocation {
    _allocation: Allocation,
    commit: super::FencedCommit,
}

/// Reject clips outside the complete viewport before converting exact local pixel coordinates.
fn clip(placement: CanvasPlacement) -> Result<D2D_RECT_F, DesktopError> {
    let viewport = placement.viewport;
    let clip = placement.clip;
    let left = i64::from(clip.x) - i64::from(viewport.x);
    let top = i64::from(clip.y) - i64::from(viewport.y);
    let right = left + i64::from(clip.width);
    let bottom = top + i64::from(clip.height);
    if viewport.width == 0
        || viewport.height == 0
        || viewport.width > crate::canvas::MAX_VIEWPORT_EDGE
        || viewport.height > crate::canvas::MAX_VIEWPORT_EDGE
        || clip.width == 0
        || clip.height == 0
        || left < 0
        || top < 0
        || right > i64::from(viewport.width)
        || bottom > i64::from(viewport.height)
        || viewport.x.unsigned_abs() > 16_777_216
        || viewport.y.unsigned_abs() > 16_777_216
    {
        return Err(DesktopError::message("canvas_placement", "clip is outside the viewport"));
    }
    #[allow(clippy::cast_precision_loss)]
    Ok(D2D_RECT_F {
        left: left as f32,
        top: top as f32,
        right: right as f32,
        bottom: bottom as f32,
    })
}

#[cfg(test)]
mod tests;
