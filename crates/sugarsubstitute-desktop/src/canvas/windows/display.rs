// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read active source-to-target display timing, preserving the OS rational refresh period.
use super::*;
use ::windows::Win32::{Devices::Display::*, Foundation::ERROR_SUCCESS};
use tao::platform::windows::MonitorHandleExtWindows;

/// Query the active path for this actual monitor; unsupported topology has an observable fallback.
pub(crate) fn refresh(monitor: &tao::monitor::MonitorHandle) -> Result<Option<f64>, DesktopError> {
    let mut paths = [DISPLAYCONFIG_PATH_INFO::default(); 64];
    let mut modes = [DISPLAYCONFIG_MODE_INFO::default(); 128];
    let mut path_count = paths.len() as u32;
    let mut mode_count = modes.len() as u32;
    // SAFETY: The two bounded output arrays remain live through the synchronous query. Counts
    // describe their actual capacities; larger/changing topologies return a reported API error.
    let status = unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(DesktopError::new(
            "canvas_display_timing",
            ::windows::core::Error::from(::windows::core::HRESULT::from_win32(status.0)),
        ));
    }
    if path_count as usize > paths.len() || mode_count as usize > modes.len() {
        return Err(DesktopError::message(
            "canvas_display_timing",
            "Native display inventory exceeds its capacity",
        ));
    }
    let identity = monitor.native_id();
    let mut result: Option<f64> = None;
    for path in &paths[..path_count as usize] {
        let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            },
            ..Default::default()
        };
        // SAFETY: The sized packet owns the complete concrete output payload across this query.
        let status = unsafe { DisplayConfigGetDeviceInfo(&mut source.header) };
        if status != 0 {
            return Err(DesktopError::new(
                "canvas_display_source",
                ::windows::core::Error::from(::windows::core::HRESULT::from_win32(status as u32)),
            ));
        }
        let end = source
            .viewGdiDeviceName
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(source.viewGdiDeviceName.len());
        if !identity.encode_utf16().eq(source.viewGdiDeviceName[..end].iter().copied()) {
            continue;
        }
        let rate = rational(
            path.targetInfo.refreshRate.Numerator,
            path.targetInfo.refreshRate.Denominator,
        )?;
        // Mirrored targets can have different physical timings; use the shared path's slower
        // pacing rather than selecting an advertised maximum that no active target can sustain.
        result = Some(result.map_or(rate, |previous| previous.min(rate)));
    }
    Ok(result)
}
fn rational(numerator: u32, denominator: u32) -> Result<f64, DesktopError> {
    let rate = f64::from(numerator) / f64::from(denominator);
    if !(1.0..=1000.0).contains(&rate) || !rate.is_finite() {
        return Err(DesktopError::message(
            "canvas_display_rate",
            "Native display timing is unavailable",
        ));
    }
    Ok(rate)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_fractional_rates_keep_rational_precision_and_invalid_timings_are_reported()
    -> Result<(), DesktopError> {
        assert_eq!(rational(60_000, 1001)?, 60_000.0 / 1001.0);
        assert_eq!(rational(144_000, 1000)?, 144.0);
        assert!(rational(0, 1).is_err());
        assert!(rational(60, 0).is_err());
        assert!(rational(1001, 1).is_err());
        Ok(())
    }
}
