// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present premultiplied CPU pixels into DWM's transparent window redirection surface.

use crate::{DesktopError, NativeMaterial, native_storage::Dib};
use std::{ffi::c_void, rc::Rc};
use sugarsubstitute_contracts::presentation::PreparedFrame;
use tao::{dpi::PhysicalPosition, platform::windows::WindowExtWindows, window::Window};
use windows::Win32::{
    Foundation::{E_ACCESSDENIED, E_INVALIDARG, E_NOTIMPL, HWND, POINT, RECT},
    Graphics::{
        Dwm::{
            DWM_BB_ENABLE, DWM_BLURBEHIND, DWMWA_SYSTEMBACKDROP_TYPE,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWINDOWATTRIBUTE,
            DwmEnableBlurBehindWindow, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
        },
        Gdi::{
            BitBlt, GetDC, GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO,
            MonitorFromPoint, ReleaseDC, SRCCOPY,
        },
    },
    UI::{Controls::MARGINS, WindowsAndMessaging::GetCursorPos},
};
#[path = "windows_frame_clip.rs"]
mod frame_clip;

/// Keep the Tao window alive until every borrowed native resource has been released.
pub(crate) struct Presenter {
    window: Rc<Window>,
    dib: Option<Dib>,
    material: NativeMaterial,
    background: u8,
    glass_top: i32,
}

impl Presenter {
    /// Report the material accepted by DWM rather than claiming unsupported effects.
    pub(crate) fn material(&self) -> NativeMaterial {
        self.material
    }
    /// Apply native material before showing the owned transparent surface.
    pub(crate) fn new(window: Rc<Window>, dark: bool) -> Result<Self, DesktopError> {
        center_on_cursor_work_area(&window)?;
        let hwnd = HWND(window.hwnd() as *mut c_void);
        let native_dark = i32::from(dark);
        let backdrop = 2_i32;
        let corner = 2_i32;
        let margins =
            MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
        let _theme_supported = set_attribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            native_dark,
            "configure_native_theme",
        )?;
        let _corners_supported = set_attribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            corner,
            "configure_native_corners",
        )?;
        let mica_supported =
            set_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, backdrop, "configure_mica")?;
        // SAFETY: Tao owns this valid HWND on the current UI thread. The margins
        // structure remains live and is synchronously copied during the call.
        unsafe {
            // Tao enables whole-window alpha for transparent windows. The presenter
            // owns the glass extent so ordinary child GDI remains opaque outside it.
            DwmEnableBlurBehindWindow(
                hwnd,
                &DWM_BLURBEHIND { dwFlags: DWM_BB_ENABLE, ..Default::default() },
            )
            .map_err(|error| DesktopError::new("claim_native_glass_extent", error))?;
            DwmExtendFrameIntoClientArea(hwnd, &margins)
                .map_err(|error| DesktopError::new("extend_native_material", error))?;
        }
        Ok(Self {
            window,
            dib: None,
            material: if mica_supported {
                NativeMaterial::Mica
            } else {
                NativeMaterial::ThemedFallback
            },
            background: if dark { 32 } else { 243 },
            glass_top: -1,
        })
    }

    /// Release copied native storage while its original frame reservation remains charged.
    pub(crate) fn shutdown(&mut self) -> Result<(), DesktopError> {
        if let Some(dib) = &mut self.dib {
            dib.close()?;
        }
        self.dib = None;
        Ok(())
    }

    /// Copy CPU BGRA bytes without replacing transparent pixels with an opaque fill.
    pub(crate) fn present(
        &mut self,
        frame: &PreparedFrame,
        excluded: Option<RECT>,
    ) -> Result<(), DesktopError> {
        let hwnd = HWND(self.window.hwnd() as *mut c_void);
        let margins = glass_margins(excluded);
        if self.glass_top != margins.cyTopHeight {
            // SAFETY: The exact retained window belongs to this UI thread. Keep
            // ordinary GDI child text outside the alpha-composited glass region.
            unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) }
                .map_err(|cause| DesktopError::new("configure_console_glass_boundary", cause))?;
            self.glass_top = margins.cyTopHeight;
        }
        let dib = Dib::for_frame(&mut self.dib, frame)?;
        let destination = dib.pixels_mut()?;
        for (source, target) in frame.pixels().chunks_exact(4).zip(destination.chunks_exact_mut(4))
        {
            if self.material == NativeMaterial::Mica {
                target.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
            } else {
                let remainder =
                    ((u16::from(self.background) * (255 - u16::from(source[3])) + 127) / 255) as u8;
                target.copy_from_slice(&[
                    source[2] + remainder,
                    source[1] + remainder,
                    source[0] + remainder,
                    255,
                ]);
            }
        }
        // SAFETY: Window and DIB handles remain valid on their creating thread. GetDC's
        // borrowed DC is released on both success and failure before returning.
        unsafe {
            let target = GetDC(Some(hwnd));
            if target.is_invalid() {
                return Err(DesktopError::message("get_window_dc", "GetDC failed"));
            }
            let result = frame_clip::submit(target, excluded, || {
                BitBlt(
                    target,
                    0,
                    0,
                    dib.width() as i32,
                    dib.height() as i32,
                    Some(dib.dc()),
                    0,
                    0,
                    SRCCOPY,
                )
                .map_err(|error| DesktopError::new("present_cpu_frame", error))
            });
            let released = ReleaseDC(Some(hwnd), target);
            result?;
            if released == 0 {
                return Err(DesktopError::message("release_window_dc", "ReleaseDC failed"));
            }
        }
        Ok(())
    }
}

/// Native GDI controls own opaque client pixels below the animated glass surface.
/// Extending glass beneath RichEdit makes its GDI alpha undefined in DWM, even
/// when a DC screenshot shows readable RGB text. Collapsing restores full glass.
fn glass_margins(console: Option<RECT>) -> MARGINS {
    match console {
        Some(rect) => {
            MARGINS { cxLeftWidth: 0, cxRightWidth: 0, cyTopHeight: rect.top, cyBottomHeight: 0 }
        }
        None => MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 },
    }
}

/// Center on the cursor monitor, using the primary monitor when cursor access is unavailable.
fn center_on_cursor_work_area(window: &Window) -> Result<(), DesktopError> {
    let mut cursor = POINT::default();
    let mut info =
        MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: Both output structures have their documented ABI and live through the
    // synchronous queries. MonitorFromPoint returns a borrowed system monitor handle.
    unsafe {
        let observed = GetCursorPos(&raw mut cursor).map(|()| cursor);
        cursor = placement_point(observed)?;
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTOPRIMARY);
        GetMonitorInfoW(monitor, &raw mut info)
            .ok()
            .map_err(|error| DesktopError::new("query_monitor_work_area", error))?;
    }
    let target_scale = window
        .available_monitors()
        .find(|monitor| {
            monitor.position() == PhysicalPosition::new(info.rcMonitor.left, info.rcMonitor.top)
        })
        .ok_or_else(|| {
            DesktopError::message(
                "query_cursor_monitor",
                "Tao did not enumerate the cursor monitor",
            )
        })?
        .scale_factor();
    let size = window
        .outer_size()
        .to_logical::<f64>(window.scale_factor())
        .to_physical::<u32>(target_scale);
    window.set_outer_position(centered_origin(info.rcWork, size.width, size.height)?);
    Ok(())
}

/// Keep cursor placement optional on an inactive desktop while preserving unexpected failures.
/// Windows denies GetCursorPos outside the input desktop; its primary monitor starts at (0, 0).
/// # Errors
/// Retains every native cursor failure other than access denied without formatting its cause.
fn placement_point(observed: windows::core::Result<POINT>) -> Result<POINT, DesktopError> {
    match observed {
        Ok(point) => Ok(point),
        Err(error) if error.code() == E_ACCESSDENIED => {
            tracing::info!(
                operation = "cursor_monitor_fallback",
                "Placing window on the primary monitor because cursor access is unavailable"
            );
            Ok(POINT::default())
        }
        Err(error) => Err(DesktopError::new("query_cursor_monitor", error)),
    }
}

/// Keep physical placement signed and bounded across monitors left of the primary screen.
fn centered_origin(
    work: RECT,
    width: u32,
    height: u32,
) -> Result<PhysicalPosition<i32>, DesktopError> {
    let span_x = i64::from(work.right) - i64::from(work.left);
    let span_y = i64::from(work.bottom) - i64::from(work.top);
    if span_x <= 0 || span_y <= 0 || width == 0 || height == 0 {
        return Err(DesktopError::message(
            "center_window",
            "work area and window must be positive",
        ));
    }
    let x = i64::from(work.left) + (span_x - i64::from(width)).div_euclid(2);
    let y = i64::from(work.top) + (span_y - i64::from(height)).div_euclid(2);
    Ok(PhysicalPosition::new(
        i32::try_from(x).map_err(|error| DesktopError::new("center_window", error))?,
        i32::try_from(y).map_err(|error| DesktopError::new("center_window", error))?,
    ))
}

/// Apply a known four-byte DWM attribute and distinguish unsupported OS support.
fn set_attribute(
    hwnd: HWND,
    attribute: DWMWINDOWATTRIBUTE,
    value: i32,
    operation: &'static str,
) -> Result<bool, DesktopError> {
    // SAFETY: The caller retains the Tao HWND on this UI thread. The attribute uses
    // its documented four-byte scalar ABI and DWM copies the live scalar synchronously.
    match unsafe { DwmSetWindowAttribute(hwnd, attribute, (&raw const value).cast(), 4) } {
        Ok(()) => Ok(true),
        Err(error) if error.code() == E_INVALIDARG || error.code() == E_NOTIMPL => {
            tracing::warn!(operation, cause = %error, "Native DWM attribute is unsupported; using the platform fallback");
            Ok(false)
        }
        Err(error) => Err(DesktopError::new(operation, error)),
    }
}

#[cfg(test)]
mod tests {
    use super::{centered_origin, glass_margins, placement_point};
    use tao::dpi::PhysicalPosition;
    use windows::Win32::Foundation::{E_ACCESSDENIED, E_INVALIDARG, POINT, RECT};

    /// Keep the entire native text/scroll region outside glass at every physical scale.
    #[test]
    fn expanded_console_ends_glass_before_gdi_text_and_collapse_restores_full_material() {
        for top in [357, 625, 714] {
            let margins =
                glass_margins(Some(RECT { left: 25, top, right: 534, bottom: top + 180 }));
            assert_eq!(margins.cyTopHeight, top);
            assert_eq!(
                (margins.cxLeftWidth, margins.cxRightWidth, margins.cyBottomHeight),
                (0, 0, 0)
            );
        }
        let collapsed = glass_margins(None);
        assert_eq!(
            (
                collapsed.cxLeftWidth,
                collapsed.cxRightWidth,
                collapsed.cyTopHeight,
                collapsed.cyBottomHeight
            ),
            (-1, -1, -1, -1)
        );
    }

    /// Preserve an available cursor and permit window placement when its desktop denies access.
    #[test]
    fn cursor_access_denied_uses_primary_monitor_without_changing_available_cursor()
    -> Result<(), Box<dyn std::error::Error>> {
        let cursor = placement_point(Ok(POINT { x: -1200, y: 350 }))?;
        assert_eq!((cursor.x, cursor.y), (-1200, 350));
        let fallback = placement_point(Err(windows::core::Error::from_hresult(E_ACCESSDENIED)))?;
        assert_eq!((fallback.x, fallback.y), (0, 0));
        Ok(())
    }

    /// Keep unexpected native cursor errors typed instead of treating them as desktop isolation.
    #[test]
    fn unexpected_cursor_failure_preserves_original_native_cause()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::error::Error;
        let failure = placement_point(Err(windows::core::Error::from_hresult(E_INVALIDARG)))
            .err()
            .ok_or("Unexpected cursor failure was accepted")?;
        let cause = failure
            .source()
            .and_then(|cause| cause.downcast_ref::<windows::core::Error>())
            .ok_or("Original typed cursor cause lost")?;
        assert_eq!(cause.code(), E_INVALIDARG);
        Ok(())
    }

    /// Preserve cursor-monitor work-area placement for negative origins and reject invalid bounds.
    #[test]
    fn work_area_centers_signed_coordinates_without_overflow()
    -> Result<(), Box<dyn std::error::Error>> {
        let work = RECT { left: -1920, top: -100, right: 0, bottom: 940 };
        assert_eq!(centered_origin(work, 558, 558)?, PhysicalPosition::new(-1239, 141));
        assert_eq!(centered_origin(work, 559, 559)?, PhysicalPosition::new(-1240, 140));
        assert!(centered_origin(RECT::default(), 558, 558).is_err());
        assert!(centered_origin(work, 0, 558).is_err());
        assert!(
            centered_origin(
                RECT { left: i32::MIN, top: 0, right: i32::MIN + 1, bottom: 1 },
                u32::MAX,
                1,
            )
            .is_err()
        );
        Ok(())
    }
}
