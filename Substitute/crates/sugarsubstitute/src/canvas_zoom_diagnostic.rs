// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One closed record per canvas zoom, relayed from the desktop child to the application log.
//!
//! The native canvas runs in the desktop child, whose output the parent relays only as records of
//! known shape. Each zoom names its wheel gap, the motion's duration and why it jumped instead of
//! animating, so an intermittently missing zoom animation names its cause in the log.
use crate::canvas_surface_diagnostic::tokens;
use sugarsubstitute_canvas::{ZoomMotion, ZoomSnap};

tokens! {
    /// Why a zoom jumped to its target, or `Animated` when it moved over time.
    Snap { Animated, Unchanged }
}

/// One zoom's motion in whole microseconds; `gap` is absent for the first wheel step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub(crate) gap: Option<u64>,
    pub(crate) duration: u64,
    pub(crate) snap: Snap,
}

impl From<ZoomMotion> for Record {
    fn from(zoom: ZoomMotion) -> Self {
        let micros =
            |value: std::time::Duration| u64::try_from(value.as_micros()).unwrap_or(u64::MAX);
        Self {
            gap: zoom.gap.map(micros),
            duration: micros(zoom.duration),
            snap: match zoom.snapped {
                None => Snap::Animated,
                Some(ZoomSnap::Unchanged) => Snap::Unchanged,
            },
        }
    }
}

/// The wheel gap as written on the wire: whole microseconds, or `none` for a first step.
struct Gap(Option<u64>);

impl std::fmt::Display for Gap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => write!(f, "{value}"),
            None => f.write_str("none"),
        }
    }
}

impl Record {
    /// Write this zoom on the child's diagnostic output.
    pub(crate) fn emit(self) {
        tracing::info!(operation = "canvas_zoom", schema = 1, gap_us = %Gap(self.gap),
            duration_us = self.duration, snap = %self.snap);
    }

    /// Accept exactly the schema-one line `emit` writes, within the child-output scratch.
    #[cfg(feature = "windows-native-host")]
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 512 {
            return None;
        }
        let text = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
        let (prefix, fields) = text.split_once("operation=\"canvas_zoom\" ")?;
        if prefix.contains("operation=")
            || !prefix.ends_with("sugarsubstitute::canvas_zoom_diagnostic: ")
        {
            return None;
        }
        let mut fields = fields.split_ascii_whitespace();
        let mut field = |name: &str| fields.next()?.strip_prefix(name)?.strip_prefix('=');
        if field("schema")? != "1" {
            return None;
        }
        let gap = match field("gap_us")? {
            "none" => None,
            value => Some(number(value)?),
        };
        let duration = number(field("duration_us")?)?;
        let snap = Snap::parse(field("snap")?)?;
        if fields.next().is_some() {
            return None;
        }
        Some(Self { gap, duration, snap })
    }

    /// Write the relayed zoom to the application log under the child's run.
    #[cfg(feature = "windows-native-host")]
    pub(crate) fn relay(self, run: sugarsubstitute_contracts::lifecycle::RunKey) {
        tracing::info!(operation = "desktop_child_canvas_zoom", run = ?run,
            gap_ms = self.gap.map(|gap| gap as f64 / 1000.0), duration_ms = self.duration as f64 / 1000.0,
            snap = %self.snap, "A zoom moved the canvas camera");
    }
}

/// Refuse signs, padding and overflow at the passive wire boundary.
#[cfg(feature = "windows-native-host")]
fn number(value: &str) -> Option<u64> {
    if value.is_empty()
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

#[cfg(all(test, feature = "windows-native-host"))]
#[path = "canvas_zoom_diagnostic/tests.rs"]
mod tests;
