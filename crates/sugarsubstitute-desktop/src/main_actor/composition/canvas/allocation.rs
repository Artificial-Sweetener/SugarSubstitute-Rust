// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one admitted composition surface and any unfinished Direct2D drawing operation.

use super::super::{adopt, missing};
use crate::{DesktopError, canvas::CanvasContent};
use std::{error::Error, fmt, mem::ManuallyDrop};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use sugarsubstitute_image_ops::Extent;
use windows::{
    Win32::{
        Foundation::{DCOMPOSITION_ERROR_SURFACE_NOT_BEING_RENDERED, POINT},
        Graphics::{
            Direct2D::{
                Common::{
                    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
                },
                D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1,
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, ID2D1Bitmap1, ID2D1DeviceContext,
            },
            DirectComposition::{IDCompositionSurface, IDCompositionSurfaceFactory},
            Dxgi::{
                Common::{DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM},
                IDXGISurface,
            },
        },
    },
    core::Interface,
};

/// Account for a surface, its update target and one uploaded bitmap.
/// Compositor- and driver-private allocations remain opaque to this process.
fn storage_bytes(extent: Extent) -> Option<u64> {
    extent.pixels().checked_mul(4)?.checked_mul(3)?.checked_add(1024 * 1024)
}

/// Surface and update resources are dropped before the claim that funds their storage.
pub(super) struct Allocation {
    extent: Extent,
    surface: Option<IDCompositionSurface>,
    drawing: bool,
    context: Option<ID2D1DeviceContext>,
    bitmap: Option<ID2D1Bitmap1>,
    #[cfg(test)]
    pub(super) capture: capture::Capture,
    _claim: MemoryClaim,
}

impl Allocation {
    /// Claim before making a native allocation; no graphics effect occurs here.
    pub(super) fn new(extent: Extent, lease: &MemoryLease) -> Result<Self, DesktopError> {
        let bytes = storage_bytes(extent)
            .ok_or_else(|| DesktopError::message("canvas_surface_budget", "extent overflow"))?;
        let claim = lease
            .claim(bytes)
            .map_err(|cause| DesktopError::new("canvas_surface_budget", cause))?;
        Ok(Self {
            extent,
            surface: None,
            drawing: false,
            context: None,
            bitmap: None,
            #[cfg(test)]
            capture: capture::Capture::default(),
            _claim: claim,
        })
    }

    /// Create a surface while retaining its transferred reference on every native status.
    pub(super) fn create(
        &mut self,
        factory: &IDCompositionSurfaceFactory,
    ) -> Result<(), DesktopError> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: The admitted dimensions and output slot remain owned by this allocation.
        let status = unsafe {
            (factory.vtable().CreateSurface)(
                factory.as_raw(),
                self.extent.width(),
                self.extent.height(),
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_ALPHA_MODE_PREMULTIPLIED,
                &mut raw,
            )
        };
        // SAFETY: The factory transfers an IDCompositionSurface reference to this slot.
        unsafe { adopt(&mut self.surface, raw, status, "create_canvas_surface") }
    }

    pub(super) fn extent(&self) -> Extent {
        self.extent
    }

    pub(super) fn surface(&self) -> Result<&IDCompositionSurface, DesktopError> {
        self.surface.as_ref().ok_or_else(|| missing("canvas_surface"))
    }

    /// Upload complete immutable pixels and always try to close the exact native draw.
    pub(super) fn upload(&mut self, content: &CanvasContent<'_>) -> Result<(), DesktopError> {
        if content.validate()? != self.extent || self.drawing {
            return Err(DesktopError::message("canvas_upload", "frame or drawing state differs"));
        }
        let result = self.draw(content);
        let retirement = self.end_draw();
        #[cfg(test)]
        let retirement = retirement.and_then(|()| self.capture.complete());
        match (result, retirement) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(cause), Ok(())) | (Ok(()), Err(cause)) => Err(cause),
            (Err(primary), Err(retirement)) => Err(DesktopError::new(
                "canvas_draw_retirement",
                DrawFailures { primary, retirement },
            )),
        }
    }

    fn draw(&mut self, content: &CanvasContent<'_>) -> Result<(), DesktopError> {
        let surface = self.surface()?.clone();
        let mut raw = std::ptr::null_mut();
        let mut offset = POINT::default();
        // A failed native mutation may still have begun; retain the surface until EndDraw succeeds.
        self.drawing = true;
        // SAFETY: The surface is live, and the exact requested interface and outputs are retained.
        let status = unsafe {
            (surface.vtable().BeginDraw)(
                surface.as_raw(),
                std::ptr::null(),
                &ID2D1DeviceContext::IID,
                &mut raw,
                &mut offset,
            )
        };
        // SAFETY: BeginDraw's transferred context is retained even after a partial failure.
        unsafe { adopt(&mut self.context, raw, status, "begin_canvas_draw")? };
        let context = self.context.as_ref().ok_or_else(|| missing("canvas_draw_context"))?;
        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            colorContext: ManuallyDrop::new(None),
        };
        let mut raw = std::ptr::null_mut();
        let status = match content {
            // SAFETY: Frame validation proves the complete immutable pixel storage and stride.
            CanvasContent::Cpu(frame) => unsafe {
                (context.vtable().CreateBitmap)(
                    context.as_raw(),
                    D2D_SIZE_U { width: self.extent.width(), height: self.extent.height() },
                    frame.pixels.as_ptr().cast(),
                    self.extent.width() * 4,
                    &properties,
                    &mut raw,
                )
            },
            CanvasContent::Shared(frame) => {
                let surface: IDXGISurface = frame
                    .texture
                    .texture()
                    .cast()
                    .map_err(|cause| DesktopError::new("query_canvas_shared_surface", cause))?;
                // SAFETY: The texture belongs to this context's D3D11 device and has the same
                // BGRA8 format; the surface outlives this call and the retained bitmap.
                unsafe {
                    (context.vtable().CreateBitmapFromDxgiSurface)(
                        context.as_raw(),
                        surface.as_raw(),
                        &properties,
                        &mut raw,
                    )
                }
            }
        };
        // SAFETY: Both creation calls transfer an ID2D1Bitmap1 reference to this retained slot.
        unsafe { adopt(&mut self.bitmap, raw, status, "create_canvas_bitmap")? };
        let bitmap = self.bitmap.as_ref().ok_or_else(|| missing("canvas_bitmap"))?;
        #[allow(clippy::cast_precision_loss)]
        let rectangle = D2D_RECT_F {
            left: offset.x as f32,
            top: offset.y as f32,
            right: (offset.x as f32) + (self.extent.width() as f32),
            bottom: (offset.y as f32) + (self.extent.height() as f32),
        };
        // SAFETY: The live context and bitmap belong to this active surface drawing operation.
        unsafe {
            context.DrawBitmap(
                bitmap,
                Some(&rectangle),
                1.0,
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
                None,
            );
        }
        #[cfg(test)]
        self.capture.copy(context, offset, self.extent)?;
        Ok(())
    }

    /// Retry unfinished draw retirement before the owning visual tree can be released.
    pub(super) fn end_draw(&mut self) -> Result<(), DesktopError> {
        if self.drawing {
            // SAFETY: The same retained surface began this drawing operation on this apartment.
            confirm_draw_ended(unsafe { self.surface()?.EndDraw() })?;
            self.drawing = false;
            self.bitmap = None;
            self.context = None;
        }
        Ok(())
    }
}

/// The native not-being-rendered result proves there is no draw left to retire.
fn confirm_draw_ended(result: windows::core::Result<()>) -> Result<(), DesktopError> {
    match result {
        Ok(()) => Ok(()),
        Err(cause) if cause.code() == DCOMPOSITION_ERROR_SURFACE_NOT_BEING_RENDERED => Ok(()),
        Err(cause) => Err(DesktopError::new("end_canvas_draw", cause)),
    }
}

#[derive(Debug)]
struct DrawFailures {
    primary: DesktopError,
    retirement: DesktopError,
}
impl fmt::Display for DrawFailures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}; drawing retirement: {}", self.primary, self.retirement)
    }
}
impl Error for DrawFailures {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.primary)
    }
}

#[cfg(test)]
mod capture;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_claim_covers_three_pixel_copies_and_overhead() -> Result<(), Box<dyn Error>> {
        let extent = Extent::new(1920, 1080)?;
        assert_eq!(storage_bytes(extent), Some(1920 * 1080 * 4 * 3 + 1024 * 1024));
        Ok(())
    }

    #[test]
    fn drawing_retirement_accepts_only_success_or_proven_absence() {
        assert!(confirm_draw_ended(Ok(())).is_ok());
        assert!(
            confirm_draw_ended(Err(windows::core::Error::from(
                DCOMPOSITION_ERROR_SURFACE_NOT_BEING_RENDERED
            )))
            .is_ok()
        );
        assert!(
            confirm_draw_ended(Err(windows::core::Error::from(windows::Win32::Foundation::E_FAIL)))
                .is_err()
        );
    }
}
