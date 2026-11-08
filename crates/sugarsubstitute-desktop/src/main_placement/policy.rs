// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve monitor-relative placement and coalesce current native samples without scheduling.

use super::MainPlacementObservation;
use crate::{DesktopError, MainIdentity};
use std::num::NonZeroU64;
use sugarsubstitute_contracts::desktop_session::{
    validate_placement,
    wire::{SessionPlacement, SessionRect},
};

/// Physical screen rectangle copied from the current native monitor/window owner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rect {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl Rect {
    /// Admit finite native coordinates before arithmetic or integer conversion.
    pub(crate) fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height].iter().all(|value| value.is_finite())
            && self.width > 0.0
            && self.height > 0.0
            && self.x >= f64::from(i32::MIN)
            && self.y >= f64::from(i32::MIN)
            && self.x + self.width <= f64::from(i32::MAX)
            && self.y + self.height <= f64::from(i32::MAX)
    }

    /// Normalize all coordinates with the same reference scale, including negative origins.
    fn logical(self, scale: f64) -> SessionRect {
        SessionRect {
            x: self.x / scale,
            y: self.y / scale,
            width: self.width / scale,
            height: self.height / scale,
        }
    }
}

/// Current monitor facts; names are opaque native device identifiers.
#[derive(Clone, Debug)]
pub(crate) struct Monitor {
    pub(crate) name: String,
    pub(crate) work: Rect,
    pub(crate) dpi: f64,
    pub(crate) primary: bool,
}

/// Checked physical normal bounds plus separate maximized reveal intent.
#[derive(Debug)]
pub(crate) struct RestorePlan {
    pub(crate) bounds: Rect,
    pub(crate) maximized: bool,
    pub(crate) adjusted: bool,
}

/// Retain the product's existing minimum logical outer size for its undecorated main window.
pub(crate) const MINIMUM_SIZE: [f64; 2] = [304.0, 240.0];

/// Resolve monitor identity first, then nearest saved physical work area with stable tie breaking.
pub(crate) fn restore(
    saved: &SessionPlacement,
    monitors: &[Monitor],
) -> Result<RestorePlan, DesktopError> {
    validate_placement(saved)
        .map_err(|cause| DesktopError::new("restore_main_placement", cause))?;
    if monitors.is_empty() || monitors.iter().any(|monitor| !valid_monitor(monitor)) {
        return Err(invalid("current monitor facts are invalid"));
    }
    let saved_scale = saved.dpi / 96.0;
    let old_center = [
        (saved.work_area.x + saved.work_area.width / 2.0) * saved_scale,
        (saved.work_area.y + saved.work_area.height / 2.0) * saved_scale,
    ];
    let distance = |monitor: &Monitor| {
        let dx = monitor.work.x + monitor.work.width / 2.0 - old_center[0];
        let dy = monitor.work.y + monitor.work.height / 2.0 - old_center[1];
        dx * dx + dy * dy
    };
    let target = monitors
        .iter()
        .find(|monitor| monitor.name == saved.monitor)
        .or_else(|| {
            monitors.iter().min_by(|left, right| {
                distance(left)
                    .total_cmp(&distance(right))
                    .then_with(|| right.primary.cmp(&left.primary))
                    .then_with(|| left.name.cmp(&right.name))
            })
        })
        .ok_or_else(|| invalid("no current monitor is available"))?;
    let scale = target.dpi / 96.0;
    let requested = Rect {
        x: target.work.x + (saved.bounds.x - saved.work_area.x) * scale,
        y: target.work.y + (saved.bounds.y - saved.work_area.y) * scale,
        width: saved.bounds.width * scale,
        height: saved.bounds.height * scale,
    };
    let width = requested
        .width
        .round()
        .max((MINIMUM_SIZE[0] * scale).ceil())
        .min(target.work.width.floor());
    let height = requested
        .height
        .round()
        .max((MINIMUM_SIZE[1] * scale).ceil())
        .min(target.work.height.floor());
    let bounds = Rect {
        x: requested.x.round().clamp(target.work.x, target.work.x + target.work.width - width),
        y: requested.y.round().clamp(target.work.y, target.work.y + target.work.height - height),
        width,
        height,
    };
    if !bounds.valid() {
        return Err(invalid("resolved geometry exceeds native coordinates"));
    }
    Ok(RestorePlan {
        bounds,
        maximized: saved.maximized,
        adjusted: bounds != requested
            || target.name != saved.monitor
            || target.dpi != saved.dpi
            || target.work.logical(scale) != saved.work_area,
    })
}

/// Capture only admitted normal screen bounds and never encode minimized presentation.
pub(crate) fn snapshot(
    bounds: Rect,
    maximized: bool,
    monitor: &Monitor,
) -> Result<SessionPlacement, DesktopError> {
    if !bounds.valid() || !valid_monitor(monitor) {
        return Err(invalid("native placement facts are invalid"));
    }
    let scale = monitor.dpi / 96.0;
    let value = SessionPlacement {
        version: 1,
        bounds: bounds.logical(scale),
        maximized,
        monitor: monitor.name.clone(),
        work_area: monitor.work.logical(scale),
        dpi: monitor.dpi,
    };
    validate_placement(&value)
        .map_err(|cause| DesktopError::new("capture_main_placement", cause))?;
    Ok(value)
}

fn valid_monitor(monitor: &Monitor) -> bool {
    monitor.work.valid()
        && monitor.work.width >= 1.0
        && monitor.work.height >= 1.0
        && (24.0..=960.0).contains(&monitor.dpi)
        && !monitor.name.is_empty()
        && monitor.name.encode_utf16().count() <= 256
        && !monitor.name.chars().any(char::is_control)
}

fn invalid(message: &'static str) -> DesktopError {
    DesktopError::message("main_placement", message)
}

/// One pending native read coalesces move/resize/DPI bursts without a scheduler or event queue.
#[derive(Default)]
pub(crate) struct Observations {
    dirty: bool,
    closed: bool,
    sequence: u64,
    last_capture: u64,
    previous: Option<SessionPlacement>,
}

impl Observations {
    /// Mark current geometry for a read between native callbacks; hidden setup does not publish.
    pub(crate) fn changed(&mut self) {
        self.dirty = !self.closed;
    }

    /// Whether a revealed owner owes at most one native sample.
    pub(crate) fn pending(&self, revealed: bool) -> bool {
        self.dirty && revealed && !self.closed
    }

    /// Stop native sampling after actual peer destruction or admission closure.
    pub(crate) fn close(&mut self) {
        self.closed = true;
        self.dirty = false;
    }

    /// Whether this observer can still sample its retained peer.
    pub(crate) fn available(&self) -> bool {
        !self.closed
    }

    /// Observe a failed read once; another actual geometry event may request a new sample.
    pub(crate) fn failed(&mut self) {
        self.dirty = false;
    }

    /// Publish changes once; explicit final capture always receives a fresh correlated sample.
    pub(crate) fn capture(
        &mut self,
        identity: MainIdentity,
        capture_id: Option<NonZeroU64>,
        placement: SessionPlacement,
    ) -> Result<Option<MainPlacementObservation>, DesktopError> {
        if self.closed || capture_id.is_some_and(|request| request.get() <= self.last_capture) {
            return Err(invalid("placement capture is closed or superseded"));
        }
        self.dirty = false;
        if capture_id.is_none() && self.previous.as_ref() == Some(&placement) {
            return Ok(None);
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .ok_or_else(|| invalid("native observation sequence exhausted"))?;
        self.sequence = sequence.get();
        if let Some(request) = capture_id {
            self.last_capture = request.get();
        }
        self.previous = Some(placement.clone());
        Ok(Some(MainPlacementObservation { identity, sequence, capture_id, placement }))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
