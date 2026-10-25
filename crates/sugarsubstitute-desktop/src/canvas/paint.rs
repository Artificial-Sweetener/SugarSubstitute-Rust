// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Blit native retained client pixels and upload only worker-prepared exposed edges.
use super::*;
use ::windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, DIB_RGB_COLORS, GetClipBox, GetDC, HDC,
        ReleaseDC, SIMPLEREGION, SRCCOPY, StretchDIBits,
    },
};
use sugarsubstitute_image_ops::Region;

/// Preserve only successful native repair dependencies; complete retained pixels remain external.
#[derive(Default)]
pub(super) struct Painter {
    last: Option<FrameStamp>,
    extent: Option<Extent>,
}
impl Painter {
    /// Upload a new frame or restore all retained pixels after native exposure.
    pub(super) fn present(
        &mut self,
        window: HWND,
        frame: &CpuFrame<'_>,
        expose: bool,
    ) -> Result<(), DesktopError> {
        frame.validate()?;
        // SAFETY: The event loop owns this live window on its native thread across acquisition,
        // synchronous painting and matched ReleaseDC. Pixel borrows remain live until return.
        let dc = unsafe { GetDC(Some(window)) };
        if dc.is_invalid() {
            return Err(DesktopError::message("canvas_get_dc", "Native canvas DC is unavailable"));
        }
        let result = self.paint(dc, frame, expose);
        // SAFETY: This is exactly the borrowed DC returned for the still-live original window.
        let release = unsafe { ReleaseDC(Some(window), dc) };
        if release == 0 {
            let cause =
                DesktopError::message("canvas_release_dc", "Native canvas DC release failed");
            return match result {
                Err(primary) => Err(DesktopError::new(
                    "canvas_paint_retirement",
                    PaintRetirement { primary, retirement: cause },
                )),
                Ok(()) => Err(cause),
            };
        }
        result
    }
    /// Paint into an already-owned DC; offscreen DIB qualification uses this same native path.
    fn paint(&mut self, dc: HDC, frame: &CpuFrame<'_>, expose: bool) -> Result<(), DesktopError> {
        let extent = frame.validate()?;
        let mut clip = RECT::default();
        // SAFETY: The caller owns this live native DC on the current presentation thread.
        let clip_kind = unsafe { GetClipBox(dc, &mut clip) };
        if clip_kind.0 == 0 {
            return Err(DesktopError::message(
                "canvas_clip",
                "Native canvas clipping query failed",
            ));
        }
        let complete = clip_kind == SIMPLEREGION
            && clip.left <= 0
            && clip.top <= 0
            && clip.right >= extent.width() as i32
            && clip.bottom >= extent.height() as i32;
        let repair = !expose
            && complete
            && self.last.is_some()
            && self.last == frame.base
            && self.extent == Some(extent);
        if repair && let Some(blit) = frame.damage.blit() {
            // SAFETY: Domain damage validates both rectangles inside this exact retained
            // viewport. GDI handles overlapping source/destination regions on the same DC.
            unsafe {
                BitBlt(
                    dc,
                    blit.destination[0] as i32,
                    blit.destination[1] as i32,
                    blit.source.width() as i32,
                    blit.source.height() as i32,
                    Some(dc),
                    blit.source.x() as i32,
                    blit.source.y() as i32,
                    SRCCOPY,
                )
            }
            .map_err(|cause| DesktopError::new("canvas_blit", cause))?;
        }
        let mut copied = true;
        if repair {
            for region in frame.damage.dirty().regions() {
                copied &= upload(dc, frame, extent, region)?;
            }
        } else {
            copied = upload(dc, frame, extent, extent.region())?;
        }
        self.last = (copied && complete).then_some(frame.target);
        self.extent = Some(extent);
        Ok(())
    }
}

/// Borrow a complete-stride top-down row window so cropped uploads use source row zero.
/// https://learn.microsoft.com/windows/win32/api/wingdi/nf-wingdi-stretchdibits
fn upload(
    dc: HDC,
    frame: &CpuFrame<'_>,
    extent: Extent,
    region: Region,
) -> Result<bool, DesktopError> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: extent.width() as i32,
            biHeight: -(region.height() as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: validate proves a complete 32-bit buffer for the bounded DIB, and every source
    // rectangle comes from the validated domain plan. This synchronous call retains no pointer.
    let copied = unsafe {
        StretchDIBits(
            dc,
            region.x() as i32,
            region.y() as i32,
            region.width() as i32,
            region.height() as i32,
            region.x() as i32,
            0,
            region.width() as i32,
            region.height() as i32,
            Some(frame.pixels.as_ptr().add(region.y() as usize * extent.width() as usize).cast()),
            &info,
            DIB_RGB_COLORS,
            SRCCOPY,
        )
    };
    if copied == -1 {
        return Err(DesktopError::message("canvas_upload", "Native canvas pixel upload failed"));
    }
    Ok(copied != 0)
}
#[derive(Debug)]
struct PaintRetirement {
    primary: DesktopError,
    retirement: DesktopError,
}
impl std::fmt::Display for PaintRetirement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}; native retirement: {}", self.primary, self.retirement)
    }
}
impl Error for PaintRetirement {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.primary)
    }
}

#[cfg(test)]
mod tests;
