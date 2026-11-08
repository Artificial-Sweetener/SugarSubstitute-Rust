// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Query and apply placement only on the thread retaining the exact native main peer.

use super::MainActor;
use crate::{DesktopError, DesktopStartupObservation, MainIdentity, main_placement};
use main_placement::{Monitor, Rect};
use std::num::NonZeroU64;
use tao::{
    platform::windows::{MonitorHandleExtWindows, WindowExtWindows},
    window::Window,
};
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{
        GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW,
        MonitorFromRect,
    },
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        WindowsAndMessaging::{
            GetWindowPlacement, IsIconic, SW_SHOWMAXIMIZED, SWP_NOACTIVATE, SWP_NOZORDER,
            SetWindowPos, ShowWindow, WINDOWPLACEMENT, WPF_RESTORETOMAXIMIZED,
        },
    },
};

/// Bound the temporary authored monitor inventory before native enumeration and allocation.
const MAX_MONITORS: usize = 64;
/// Monitor records, their bounded UTF-8 names, previous/current samples and output storage.
pub(super) const STORAGE_BYTES: u64 = 128 * 1024;

impl MainActor {
    /// Resolve saved normal bounds after backdrop centering, before children and first reveal.
    pub(super) fn restore_placement(&self) -> Result<(), DesktopError> {
        let Some(saved) = self.request()?.initial_placement() else {
            return Ok(());
        };
        let window = self.window()?;
        let mut monitors = Vec::with_capacity(MAX_MONITORS);
        for monitor in window.available_monitors() {
            if monitors.len() == MAX_MONITORS {
                return Err(DesktopError::message(
                    "restore_main_placement",
                    "monitor inventory exceeds admission",
                ));
            }
            monitors.push(monitor_facts(HMONITOR(monitor.hmonitor() as *mut _))?.0);
        }
        let plan = main_placement::restore(saved, &monitors)?;
        let bounds = plan.bounds;
        let hwnd = hwnd(window);
        // SAFETY: This actor owns the hidden peer on its creating thread; bounds were checked
        // against finite integer coordinates and current work area before native conversion.
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                bounds.x as i32,
                bounds.y as i32,
                bounds.width as i32,
                bounds.height as i32,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
            .map_err(|cause| DesktopError::new("apply_main_placement", cause))?;
        }
        tracing::info!(operation="restore_main_placement", identity=?self.identity(),
            adjusted=plan.adjusted, maximized=plan.maximized,
            "Applied native normal placement before reveal");
        Ok(())
    }

    /// Reveal directly in the saved show state while preserving the applied normal bounds.
    pub(super) fn show_placed_window(&self) -> Result<(), DesktopError> {
        let window = self.window()?;
        if self.request()?.initial_placement().is_some_and(|placement| placement.maximized) {
            // Tao's hidden maximize sets WS_MAXIMIZE before Windows records the normal
            // rectangle. Showing maximized through Win32 preserves the already placed HWND;
            // synchronous WM_SIZE updates Tao's intent before its visible flag is published.
            // SAFETY: This actor owns the original hidden HWND on the creating UI thread.
            let _previous_visibility = unsafe { ShowWindow(hwnd(window), SW_SHOWMAXIMIZED) };
        }
        window.set_visible(true);
        Ok(())
    }

    /// Read one pending changed placement between callbacks, while retaining the native peer.
    pub(super) fn placement_step(
        &mut self,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        if !self.placement.pending(self.revealed) {
            return Ok(None);
        }
        let identity = self.identity();
        let sample = match self.window().and_then(|window| capture(window)) {
            Ok(sample) => sample,
            Err(cause) => {
                self.placement.failed();
                return Ok(Some(DesktopStartupObservation::MainPlacementCaptureFailed {
                    identity,
                    capture_id: None,
                    cause,
                }));
            }
        };
        Ok(self
            .placement
            .capture(identity, None, sample)?
            .map(DesktopStartupObservation::MainPlacementObserved))
    }

    /// Return exactly one correlated terminal sample without closing the actor on getter failure.
    pub(crate) fn capture_placement(
        &mut self,
        identity: MainIdentity,
        capture_id: NonZeroU64,
    ) -> DesktopStartupObservation {
        let result = (|| {
            if identity != self.identity()
                || self.admission.closing()
                || self.retired
                || !self.revealed
                || !self.placement.available()
            {
                return Err(DesktopError::message(
                    "capture_main_placement",
                    "native lifetime is stale or unavailable",
                ));
            }
            let sample = capture(self.window()?)?;
            self.placement.capture(identity, Some(capture_id), sample)?.ok_or_else(|| {
                DesktopError::message("capture_main_placement", "explicit sample missing")
            })
        })();
        match result {
            Ok(sample) => DesktopStartupObservation::MainPlacementObserved(sample),
            Err(cause) => DesktopStartupObservation::MainPlacementCaptureFailed {
                identity,
                capture_id: Some(capture_id),
                cause,
            },
        }
    }
}

/// Convert the current normal restore rectangle, not maximized or minimized screen dimensions.
fn capture(
    window: &Window,
) -> Result<sugarsubstitute_contracts::desktop_session::wire::SessionPlacement, DesktopError> {
    let hwnd = hwnd(window);
    let mut placement =
        WINDOWPLACEMENT { length: size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
    // SAFETY: The caller retains this exact live HWND on its creating thread; the output is local.
    unsafe { GetWindowPlacement(hwnd, &raw mut placement) }
        .map_err(|cause| DesktopError::new("query_main_placement", cause))?;
    // SAFETY: Use the normal rectangle's monitor even when the current maximized/minimized
    // presentation belongs elsewhere; the local rectangle outlives this synchronous query.
    let normal_monitor =
        unsafe { MonitorFromRect(&raw const placement.rcNormalPosition, MONITOR_DEFAULTTONEAREST) };
    let (_, info) = monitor_facts(normal_monitor)?;
    let normal = workspace_to_screen(placement.rcNormalPosition, info)?;
    let screen = RECT {
        left: normal.x as i32,
        top: normal.y as i32,
        right: (normal.x + normal.width) as i32,
        bottom: (normal.y + normal.height) as i32,
    };
    // SAFETY: The finite screen rectangle remains alive throughout the synchronous query.
    let reference = unsafe { MonitorFromRect(&raw const screen, MONITOR_DEFAULTTONEAREST) };
    let (monitor, _) = monitor_facts(reference)?;
    // SAFETY: Same retained native peer; scalar state does not transfer handle custody.
    let minimized = unsafe { IsIconic(hwnd) }.as_bool();
    let maximized = maximized_intent(&placement, minimized);
    main_placement::snapshot(normal, maximized, &monitor)
}

/// Minimized presentation retains the restore-to-maximized flag, never minimized dimensions/state.
fn maximized_intent(placement: &WINDOWPLACEMENT, minimized: bool) -> bool {
    placement.showCmd == SW_SHOWMAXIMIZED.0 as u32
        || (minimized && placement.flags.contains(WPF_RESTORETOMAXIMIZED))
}

fn hwnd(window: &Window) -> HWND {
    HWND(window.hwnd() as *mut _)
}

/// Copy native device/work-area/DPI values; the borrowed HMONITOR never escapes this adapter.
fn monitor_facts(handle: HMONITOR) -> Result<(Monitor, MONITORINFO), DesktopError> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    let mut dpi = [0_u32; 2];
    // SAFETY: The EX structure starts with MONITORINFO and its size requests the device-name
    // extension. Both output structures outlive the synchronous monitor queries.
    unsafe {
        GetMonitorInfoW(handle, &raw mut info.monitorInfo)
            .ok()
            .map_err(|cause| DesktopError::new("query_main_monitor", cause))?;
        GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &raw mut dpi[0], &raw mut dpi[1])
            .map_err(|cause| DesktopError::new("query_main_monitor_dpi", cause))?;
    }
    let length = info.szDevice.iter().position(|unit| *unit == 0).ok_or_else(|| {
        DesktopError::message("query_main_monitor", "monitor name is unterminated")
    })?;
    let name = String::from_utf16(&info.szDevice[..length])
        .map_err(|cause| DesktopError::new("query_main_monitor", cause))?;
    let area = info.monitorInfo.rcWork;
    let work = Rect {
        x: f64::from(area.left),
        y: f64::from(area.top),
        width: f64::from(area.right) - f64::from(area.left),
        height: f64::from(area.bottom) - f64::from(area.top),
    };
    Ok((
        Monitor { name, work, dpi: f64::from(dpi[0]), primary: info.monitorInfo.dwFlags & 1 != 0 },
        info.monitorInfo,
    ))
}

/// WINDOWPLACEMENT uses workspace coordinates for this ordinary top-level non-tool window.
/// Add only the work-area inset relative to its monitor; absolute monitor origins stay intact.
fn workspace_to_screen(normal: RECT, monitor: MONITORINFO) -> Result<Rect, DesktopError> {
    let x =
        f64::from(normal.left) + f64::from(monitor.rcWork.left) - f64::from(monitor.rcMonitor.left);
    let y =
        f64::from(normal.top) + f64::from(monitor.rcWork.top) - f64::from(monitor.rcMonitor.top);
    let bounds = Rect {
        x,
        y,
        width: f64::from(normal.right) - f64::from(normal.left),
        height: f64::from(normal.bottom) - f64::from(normal.top),
    };
    if !bounds.valid() {
        return Err(DesktopError::message("query_main_placement", "normal rectangle is invalid"));
    }
    Ok(bounds)
}

#[cfg(test)]
mod tests;
