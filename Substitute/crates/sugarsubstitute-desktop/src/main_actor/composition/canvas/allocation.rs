// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one admitted composition surface and any unfinished Direct2D drawing operation.

use super::{
    super::{adopt, missing},
    timing::{Reach, Source, UploadCost},
};
use crate::{
    DesktopError,
    canvas::{CanvasContent, CpuFrame},
};
use std::{
    error::Error,
    fmt,
    mem::ManuallyDrop,
    time::{Duration, Instant},
};
use sugarsubstitute_canvas::{DamagePlan, FrameStamp};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use sugarsubstitute_image_ops::{Extent, Region};
use windows::{
    Win32::{
        Foundation::{DCOMPOSITION_ERROR_SURFACE_NOT_BEING_RENDERED, POINT, RECT},
        Graphics::{
            Direct2D::{
                Common::{
                    D2D_RECT_F, D2D_RECT_U, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED,
                    D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
                },
                D2D1_ANTIALIAS_MODE_ALIASED, D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1,
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
    /// Wraps a shared texture for one draw only.
    bitmap: Option<ID2D1Bitmap1>,
    /// Device pixels of this surface's extent that every CPU frame refills, so a preview
    /// replaces pixels without allocating. Dropped whenever a draw fails.
    upload: Option<ID2D1Bitmap1>,
    /// The frame whose complete pixels the retained bitmap is known to hold. A shared frame never
    /// touches the bitmap, so this may be older than the frame on screen; it is unknown after a
    /// failure and after a repair that moved pixels the bitmap did not move.
    upload_holds: Option<FrameStamp>,
    /// The frame whose complete pixels the composition surface is known to retain: the last
    /// frame whose every draw was closed. Unknown before the first full draw and after any
    /// failure. A commit acknowledgement says nothing about this, nor this about the bitmap.
    surface_holds: Option<FrameStamp>,
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
            upload: None,
            upload_holds: None,
            surface_holds: None,
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

    /// The frames the composition surface and the upload bitmap are known to hold.
    #[cfg(test)]
    pub(super) fn holds(&self) -> (Option<FrameStamp>, Option<FrameStamp>) {
        (self.surface_holds, self.upload_holds)
    }

    pub(super) fn surface(&self) -> Result<&IDCompositionSurface, DesktopError> {
        self.surface.as_ref().ok_or_else(|| missing("canvas_surface"))
    }

    /// Show one frame and always try to close every native draw it began.
    ///
    /// A CPU frame whose `base` is the frame this surface retains is shown by repairing the
    /// surface: retained pixels move if its plan moves them, and only its changed regions are
    /// redrawn. Every other frame clears and draws the whole surface: the first frame, a frame
    /// after a shared frame the plan is not relative to, a skipped generation, a whole-viewport
    /// change, and any frame after a failure.
    pub(super) fn upload(
        &mut self,
        content: &CanvasContent<'_>,
        wash: [u8; 4],
    ) -> Result<UploadCost, DesktopError> {
        if content.validate()? != self.extent || self.drawing {
            return Err(DesktopError::message("canvas_upload", "frame or drawing state differs"));
        }
        let result = match self.repairable(content) {
            Some(frame) => self.repair(frame, wash),
            None => self.draw(content, wash),
        };
        let retirement = self.end_draw();
        #[cfg(test)]
        let retirement = retirement.and_then(|()| self.capture.complete());
        if result.is_err() || retirement.is_err() {
            // Neither retained store is known any more; the next frame rebuilds both.
            self.upload = None;
            self.upload_holds = None;
            self.surface_holds = None;
        }
        match (result, retirement) {
            (Ok(filled), Ok(())) => {
                self.surface_holds = Some(content.target());
                Ok(UploadCost {
                    source: filled.source,
                    source_bytes: filled.source_bytes,
                    open: filled.open,
                    fill: filled.fill,
                    draw: filled.draw
                        + filled.drawing_since.map_or(Duration::ZERO, |since| since.elapsed()),
                    reach: filled.reach,
                })
            }
            (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
            (Err(primary), Err(retirement)) => Err(DesktopError::new(
                "canvas_draw_retirement",
                DrawFailures { primary, retirement },
            )),
        }
    }

    /// The CPU frame to show by repair: its plan is relative to exactly the frame this surface
    /// retains and leaves part of the viewport as it is.
    fn repairable<'a, 'b>(&self, content: &'a CanvasContent<'b>) -> Option<&'a CpuFrame<'b>> {
        let CanvasContent::Cpu(frame) = content else { return None };
        let held = self.surface_holds?;
        (frame.base == Some(held) && !whole(frame.damage, self.extent)).then_some(frame)
    }

    /// Bring the retained bitmap to `frame`'s pixels, copying only its changed regions when the
    /// bitmap holds the frame those regions are relative to.
    fn fill(
        &mut self,
        context: &ID2D1DeviceContext,
        frame: &CpuFrame<'_>,
    ) -> Result<(Source, u64), DesktopError> {
        let stride = self.extent.width() * 4;
        let extent = self.extent;
        let based = frame.base.is_some()
            && self.upload_holds == frame.base
            && frame.damage.blit().is_none();
        let complete = size_of_val(frame.pixels) as u64;
        let refilled = match &self.upload {
            Some(bitmap) if based => {
                let mut bytes = 0;
                frame
                    .damage
                    .dirty()
                    .regions()
                    .all(|region| {
                        bytes += region.pixels() * 4;
                        copy_region(bitmap, frame.pixels, extent, region).is_ok()
                    })
                    .then_some(bytes)
            }
            // SAFETY: Frame validation proves the complete immutable pixel storage and stride;
            // the retained bitmap has this surface's extent and format.
            Some(bitmap) => unsafe {
                bitmap.CopyFromMemory(None, frame.pixels.as_ptr().cast(), stride).is_ok()
            }
            .then_some(complete),
            None => None,
        };
        self.upload_holds = None;
        let filled = match refilled {
            Some(bytes) => (Source::Updated, bytes),
            None => {
                self.upload = None;
                let mut raw = std::ptr::null_mut();
                // SAFETY: As above; the context retains nothing of the pixel storage.
                let status = unsafe {
                    (context.vtable().CreateBitmap)(
                        context.as_raw(),
                        D2D_SIZE_U { width: extent.width(), height: extent.height() },
                        frame.pixels.as_ptr().cast(),
                        stride,
                        &bitmap_properties(),
                        &mut raw,
                    )
                };
                // SAFETY: CreateBitmap transfers an ID2D1Bitmap1 reference to this slot.
                unsafe { adopt(&mut self.upload, raw, status, "create_canvas_bitmap")? };
                (Source::Created, complete)
            }
        };
        self.upload_holds = Some(frame.target);
        Ok(filled)
    }

    /// Open a surface update for `area`, or for the whole surface, and return where to draw it.
    fn begin(&mut self, area: Option<Region>) -> Result<POINT, DesktopError> {
        let surface = self.surface()?.clone();
        let area = area.map(rectangle);
        let mut raw = std::ptr::null_mut();
        let mut offset = POINT::default();
        // A failed native mutation may still have begun; retain the surface until EndDraw succeeds.
        self.drawing = true;
        // SAFETY: The surface is live, and the exact requested interface and outputs are retained.
        let status = unsafe {
            (surface.vtable().BeginDraw)(
                surface.as_raw(),
                area.as_ref().map_or(std::ptr::null(), std::ptr::from_ref),
                &ID2D1DeviceContext::IID,
                &mut raw,
                &mut offset,
            )
        };
        // SAFETY: BeginDraw's transferred context is retained even after a partial failure.
        unsafe { adopt(&mut self.context, raw, status, "begin_canvas_draw")? };
        Ok(offset)
    }

    /// Move the surface's retained pixels where the plan moves them, then redraw only its
    /// changed regions, each in its own surface update. A plan that changes no pixel draws
    /// nothing: the retained surface already is the frame.
    fn repair(&mut self, frame: &CpuFrame<'_>, wash: [u8; 4]) -> Result<Filled, DesktopError> {
        let started = Instant::now();
        let plan = frame.damage;
        if let Some(blit) = plan.blit() {
            let source = rectangle(blit.source);
            let across = i64::from(blit.destination[0]) - i64::from(blit.source.x());
            let down = i64::from(blit.destination[1]) - i64::from(blit.source.y());
            let (Ok(across), Ok(down)) = (i32::try_from(across), i32::try_from(down)) else {
                return Err(DesktopError::message("scroll_canvas_surface", "movement overflow"));
            };
            // SAFETY: The surface is live and not being drawn; the source lies within it.
            unsafe { self.surface()?.Scroll(Some(&source), None, across, down) }
                .map_err(|cause| DesktopError::new("scroll_canvas_surface", cause))?;
        }
        // The bitmap follows the frame only when it held the plan's base and nothing moved.
        let follows = self.upload.is_some()
            && self.upload_holds == frame.base
            && plan.blit().is_none();
        let extent = self.extent;
        let mut open = started.elapsed();
        let mut fill = Duration::ZERO;
        let mut draw = Duration::ZERO;
        let mut source = Source::Updated;
        let mut source_bytes = 0;
        let mut regions = 0;
        let mut created = false;
        self.upload_holds = None;
        for region in plan.dirty().regions() {
            let opening = Instant::now();
            let offset = self.begin(Some(region))?;
            open += opening.elapsed();
            let filling = Instant::now();
            let context =
                self.context.clone().ok_or_else(|| missing("canvas_draw_context"))?;
            if self.upload.is_none() {
                // The first repair after a shared frame has no bitmap yet: it starts complete.
                (source, source_bytes) = self.fill_complete(&context, frame)?;
                created = true;
            } else if !created {
                let bitmap = self.upload.as_ref().ok_or_else(|| missing("canvas_bitmap"))?;
                copy_region(bitmap, frame.pixels, extent, region)
                    .map_err(|cause| DesktopError::new("update_canvas_bitmap", cause))?;
                source_bytes += region.pixels() * 4;
            }
            let bitmap = self.upload.as_ref().ok_or_else(|| missing("canvas_bitmap"))?;
            fill += filling.elapsed();
            let drawing = Instant::now();
            #[allow(clippy::cast_precision_loss)]
            let target = D2D_RECT_F {
                left: offset.x as f32,
                top: offset.y as f32,
                right: offset.x as f32 + region.width() as f32,
                bottom: offset.y as f32 + region.height() as f32,
            };
            #[allow(clippy::cast_precision_loss)]
            let from = D2D_RECT_F {
                left: region.x() as f32,
                top: region.y() as f32,
                right: region.right() as f32,
                bottom: region.bottom() as f32,
            };
            #[cfg(test)]
            self.capture.prefill(&context, &target);
            paint(&context, bitmap, &target, Some(&from), wash);
            #[cfg(test)]
            self.capture.copy(&context, offset, Extent::new(region.width(), region.height())
                .map_err(|cause| DesktopError::new("capture_canvas_surface", cause))?)?;
            drop(context);
            self.end_draw()?;
            draw += drawing.elapsed();
            regions += 1;
        }
        if created || follows {
            self.upload_holds = Some(frame.target);
        }
        Ok(Filled {
            source,
            source_bytes,
            open,
            fill,
            draw,
            drawing_since: None,
            reach: Reach::Repaired { scrolled: plan.blit().is_some(), regions },
        })
    }

    /// Replace the retained bitmap with one created from the frame's complete pixels.
    fn fill_complete(
        &mut self,
        context: &ID2D1DeviceContext,
        frame: &CpuFrame<'_>,
    ) -> Result<(Source, u64), DesktopError> {
        self.upload = None;
        self.upload_holds = None;
        self.fill(context, frame)
    }

    fn draw(&mut self, content: &CanvasContent<'_>, wash: [u8; 4]) -> Result<Filled, DesktopError> {
        let started = Instant::now();
        let offset = self.begin(None)?;
        let open = started.elapsed();
        let filling = Instant::now();
        let context = self.context.clone().ok_or_else(|| missing("canvas_draw_context"))?;
        let (source, source_bytes) = match content {
            CanvasContent::Cpu(frame) => self.fill(&context, frame)?,
            CanvasContent::Shared(frame) => {
                let surface: IDXGISurface = frame
                    .texture
                    .texture()
                    .cast()
                    .map_err(|cause| DesktopError::new("query_canvas_shared_surface", cause))?;
                let mut raw = std::ptr::null_mut();
                // SAFETY: The texture belongs to this context's D3D11 device and has the same
                // BGRA8 format; the surface outlives this call and the retained bitmap.
                let status = unsafe {
                    (context.vtable().CreateBitmapFromDxgiSurface)(
                        context.as_raw(),
                        surface.as_raw(),
                        &bitmap_properties(),
                        &mut raw,
                    )
                };
                // SAFETY: The call transfers an ID2D1Bitmap1 reference to this retained slot.
                unsafe { adopt(&mut self.bitmap, raw, status, "create_canvas_bitmap")? };
                (Source::Wrapped, 0)
            }
        };
        let bitmap = match source {
            Source::Wrapped => self.bitmap.as_ref(),
            Source::Created | Source::Updated => self.upload.as_ref(),
        }
        .ok_or_else(|| missing("canvas_bitmap"))?;
        let fill = filling.elapsed();
        let drawing_since = Instant::now();
        #[allow(clippy::cast_precision_loss)]
        let rectangle = D2D_RECT_F {
            left: offset.x as f32,
            top: offset.y as f32,
            right: (offset.x as f32) + (self.extent.width() as f32),
            bottom: (offset.y as f32) + (self.extent.height() as f32),
        };
        #[cfg(test)]
        self.capture.prefill(&context, &rectangle);
        paint(&context, bitmap, &rectangle, None, wash);
        #[cfg(test)]
        self.capture.copy(&context, offset, self.extent)?;
        Ok(Filled {
            source,
            source_bytes,
            open,
            fill,
            draw: Duration::ZERO,
            drawing_since: Some(drawing_since),
            reach: Reach::Whole,
        })
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

/// Pixels handed to Direct2D for one frame.
struct Filled {
    source: Source,
    source_bytes: u64,
    /// Opening the surface updates.
    open: Duration,
    /// Handing pixels over.
    fill: Duration,
    /// Drawing already closed.
    draw: Duration,
    /// When the surface update that is still open began drawing.
    drawing_since: Option<Instant>,
    reach: Reach,
}

/// Whether a plan redraws the complete viewport, leaving nothing retained to repair.
fn whole(plan: &DamagePlan, extent: Extent) -> bool {
    plan.blit().is_none() && plan.dirty().regions().eq([extent.region()])
}

fn rectangle(region: Region) -> RECT {
    #[allow(clippy::cast_possible_wrap)]
    RECT {
        left: region.x() as i32,
        top: region.y() as i32,
        right: region.right() as i32,
        bottom: region.bottom() as i32,
    }
}

fn bitmap_properties() -> D2D1_BITMAP_PROPERTIES1 {
    D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
        colorContext: ManuallyDrop::new(None),
    }
}

/// Copy one region of a complete frame into the same region of the retained bitmap.
fn copy_region(
    bitmap: &ID2D1Bitmap1,
    pixels: &[u32],
    extent: Extent,
    region: Region,
) -> windows::core::Result<()> {
    let area = D2D_RECT_U {
        left: region.x(),
        top: region.y(),
        right: region.right(),
        bottom: region.bottom(),
    };
    let first = region.y() as usize * extent.width() as usize + region.x() as usize;
    // SAFETY: Plan regions are clipped to this extent and frame validation proves `pixels` is the
    // complete extent, so every row of the region, read at the frame's stride from its first
    // pixel, lies inside the slice; the bitmap has this extent and format.
    unsafe { bitmap.CopyFromMemory(Some(&area), pixels[first..].as_ptr().cast(), extent.width() * 4) }
}

/// Replace `target` of the open surface update with the desktop body wash under `bitmap`.
///
/// DirectComposition leaves an update rectangle's prior contents undefined, often an earlier
/// frame. Premultiplied frames carry transparent pixels, so source-over drawing would let those
/// contents show; clearing first makes every draw replace what it covers. The clear is the
/// desktop body wash, so the window's Mica shows through it behind the image as it does behind
/// the page's workspace.
fn paint(
    context: &ID2D1DeviceContext,
    bitmap: &ID2D1Bitmap1,
    target: &D2D_RECT_F,
    from: Option<&D2D_RECT_F>,
    wash: [u8; 4],
) {
    let [red, green, blue, alpha] = wash.map(|channel| f32::from(channel) / 255.0);
    // SAFETY: The live context and bitmap belong to this active surface drawing operation; the
    // aliased clip is the exact update rectangle and is popped before drawing.
    unsafe {
        context.PushAxisAlignedClip(target, D2D1_ANTIALIAS_MODE_ALIASED);
        context.Clear(Some(&D2D1_COLOR_F { r: red, g: green, b: blue, a: alpha }));
        context.PopAxisAlignedClip();
        context.DrawBitmap(
            bitmap,
            Some(target),
            1.0,
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
            from.map(std::ptr::from_ref),
            None,
        );
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
