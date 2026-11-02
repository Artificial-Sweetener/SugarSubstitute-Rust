// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read active source-to-target display timing, preserving the OS rational refresh period.
use super::*;
use ::windows::Win32::{Devices::Display::*, Foundation::ERROR_SUCCESS};
use tao::platform::windows::MonitorHandleExtWindows;

/// Active display paths and modes from one bounded native topology query.
struct Topology {
    paths: [DISPLAYCONFIG_PATH_INFO; 64],
    modes: [DISPLAYCONFIG_MODE_INFO; 128],
    path_count: usize,
    mode_count: usize,
}

impl Topology {
    fn query(operation: &'static str) -> Result<Self, DesktopError> {
        let mut paths = [DISPLAYCONFIG_PATH_INFO::default(); 64];
        let mut modes = [DISPLAYCONFIG_MODE_INFO::default(); 128];
        let mut path_count = paths.len() as u32;
        let mut mode_count = modes.len() as u32;
        // SAFETY: The two bounded output arrays remain live through the synchronous query.
        // Counts describe their actual capacities; larger/changing topologies return a
        // reported API error.
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
                operation,
                ::windows::core::Error::from(::windows::core::HRESULT::from_win32(status.0)),
            ));
        }
        if path_count as usize > paths.len() || mode_count as usize > modes.len() {
            return Err(DesktopError::message(
                operation,
                "Native display inventory exceeds its capacity",
            ));
        }
        Ok(Self { paths, modes, path_count: path_count as usize, mode_count: mode_count as usize })
    }

    fn paths(&self) -> &[DISPLAYCONFIG_PATH_INFO] {
        &self.paths[..self.path_count]
    }

    fn modes(&self) -> &[DISPLAYCONFIG_MODE_INFO] {
        &self.modes[..self.mode_count]
    }
}

/// Physical width and height of the box enclosing every active display source.
/// Desktop coordinates are physical pixels independent of this process's DPI awareness, so
/// no window on the attached displays can show a larger canvas.
pub(crate) fn desktop_extent() -> Result<[u32; 2], DesktopError> {
    let topology = Topology::query("canvas_desktop_extent")?;
    let sources = topology
        .modes()
        .iter()
        .filter(|mode| mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE)
        .map(|mode| {
            // SAFETY: Source-typed mode records carry the source member of their union.
            let source = unsafe { mode.Anonymous.sourceMode };
            ([source.position.x, source.position.y], [source.width, source.height])
        });
    bounds(sources).ok_or_else(|| {
        DesktopError::message("canvas_desktop_extent", "No active display source is attached")
    })
}

/// Enclose `(origin, size)` rectangles, saturating at the largest representable size.
fn bounds(rectangles: impl IntoIterator<Item = ([i32; 2], [u32; 2])>) -> Option<[u32; 2]> {
    let mut enclosing: Option<[i64; 4]> = None;
    for ([x, y], [width, height]) in rectangles {
        let rectangle = [
            i64::from(x),
            i64::from(y),
            i64::from(x) + i64::from(width),
            i64::from(y) + i64::from(height),
        ];
        enclosing = Some(enclosing.map_or(rectangle, |current| {
            [
                current[0].min(rectangle[0]),
                current[1].min(rectangle[1]),
                current[2].max(rectangle[2]),
                current[3].max(rectangle[3]),
            ]
        }));
    }
    let [left, top, right, bottom] = enclosing?;
    let edge = |span: i64| u32::try_from(span).unwrap_or(u32::MAX);
    Some([edge(right - left), edge(bottom - top)])
        .filter(|[width, height]| *width > 0 && *height > 0)
}

/// Query the active path for this actual monitor; unsupported topology has an observable fallback.
pub(crate) fn refresh(monitor: &tao::monitor::MonitorHandle) -> Result<Option<f64>, DesktopError> {
    let topology = Topology::query("canvas_display_timing")?;
    let identity = monitor.native_id();
    let mut result: Option<f64> = None;
    for path in topology.paths() {
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
    fn desktop_bounds_enclose_offset_and_negative_displays_or_report_none() {
        assert_eq!(bounds([]), None);
        assert_eq!(bounds([([0, 0], [3440, 1440])]), Some([3440, 1440]));
        let side_by_side = [([0, 0], [2560, 1440]), ([-1920, 360], [1920, 1080])];
        assert_eq!(bounds(side_by_side), Some([4480, 1440]));
        assert_eq!(bounds([([0, 0], [0, 0])]), None);
    }
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
