// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One closed record per camera motion, relayed from the desktop child to the application log.
//!
//! A camera motion (Fit, 1:1, wheel) should present a frame every display period until it
//! settles. Each motion records how long it ran, the display period it paced to, how many frames
//! the native queue accepted, the longest gap between them and whether detail work was running
//! when it began, so a choppy or missing animation names what it presented.
use std::time::Duration;
use sugarsubstitute_canvas_render::PresentationWindow;

/// What one camera motion presented, in whole microseconds and frame counts. Fields are 32-bit,
/// saturating (71 minutes of microseconds), so the relayed observation stays within its inline
/// metadata size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub(crate) elapsed: u32,
    pub(crate) period: u32,
    /// From the motion's start to its first accepted frame; absent when none was accepted.
    pub(crate) first_frame: Option<u32>,
    pub(crate) accepted: u32,
    pub(crate) submitted: u32,
    pub(crate) abandoned: u32,
    pub(crate) skipped: u32,
    pub(crate) longest_gap: u32,
    pub(crate) detail: bool,
    /// Ticks each gate stopped without a frame, in `GATES` order: the display gates (a frame
    /// already preparing, GPU presentation pending, a CPU document frame pending, a frame in
    /// flight at the pacer, no backing, nothing new to show, no display buffer of the view's
    /// size), then session work that ended the tick before display. 16-bit, saturating.
    pub(crate) blocked: [u16; 17],
}

/// The wire names of the blocked counts, in `Record::blocked` order: the display gates (frame
/// preparing, GPU pending, CPU pending, pacer, no backing, nothing new, buffers), then the tick
/// holds (retiring, restoring, acquisition, initializing, picker, GPU init, input route,
/// opening, suspended, stale scene). Short, so a record keeps within the relay line.
const GATES: [&str; 17] = [
    "disp", "gpu", "cpu", "pace", "back", "same", "buf", "ret", "rest", "acq", "init", "pick",
    "ginit", "route", "open", "susp", "stale",
];

/// Whole microseconds, saturating.
fn micros(value: Duration) -> u32 {
    u32::try_from(value.as_micros()).unwrap_or(u32::MAX)
}

/// A count, saturating.
fn count(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

impl Record {
    /// The record of `window`, closed at `now`, paced to `period`.
    pub(crate) fn new(
        window: PresentationWindow,
        now: Duration,
        period: Duration,
        detail: bool,
        blocked: [u64; 17],
    ) -> Self {
        let stats = window.stats;
        Self {
            elapsed: micros(now.saturating_sub(window.started)),
            period: micros(period),
            first_frame: window
                .first_acceptance
                .map(|at| micros(at.saturating_sub(window.started))),
            accepted: count(stats.canonical_accepted.saturating_add(stats.projected_accepted)),
            submitted: count(stats.submitted),
            abandoned: count(stats.abandoned),
            skipped: count(stats.skipped_slots),
            longest_gap: micros(stats.native_acceptance_cadence.maximum),
            detail,
            blocked: blocked.map(|value| u16::try_from(value).unwrap_or(u16::MAX)),
        }
    }

    /// Write this motion on the child's diagnostic output.
    pub(crate) fn emit(self) {
        tracing::info!(operation = "canvas_motion", schema = 1, elapsed_us = self.elapsed,
            period_us = self.period, first_frame_us = %Optional(self.first_frame),
            accepted = self.accepted, submitted = self.submitted, abandoned = self.abandoned,
            skipped = self.skipped, longest_gap_us = self.longest_gap, detail = self.detail,
            blocked = %Counts(self.blocked));
    }

    /// Accept exactly the schema-one line `emit` writes, within the child-output scratch.
    #[cfg(feature = "windows-native-host")]
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 512 {
            return None;
        }
        let text = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
        let (prefix, fields) = text.split_once("operation=\"canvas_motion\" ")?;
        if prefix.contains("operation=")
            || !prefix.ends_with("sugarsubstitute::canvas_motion_diagnostic: ")
        {
            return None;
        }
        let mut fields = fields.split_ascii_whitespace();
        let mut field = |name: &str| fields.next()?.strip_prefix(name)?.strip_prefix('=');
        if field("schema")? != "1" {
            return None;
        }
        let elapsed = number(field("elapsed_us")?)?;
        let period = number(field("period_us")?)?;
        let first_frame = match field("first_frame_us")? {
            "none" => None,
            value => Some(number(value)?),
        };
        let accepted = number(field("accepted")?)?;
        let submitted = number(field("submitted")?)?;
        let abandoned = number(field("abandoned")?)?;
        let skipped = number(field("skipped")?)?;
        let longest_gap = number(field("longest_gap_us")?)?;
        let detail = match field("detail")? {
            "true" => true,
            "false" => false,
            _ => return None,
        };
        let mut blocked = [0; 17];
        let mut counts = field("blocked")?.split(',');
        for (slot, name) in blocked.iter_mut().zip(GATES) {
            *slot = u16::try_from(number(counts.next()?.strip_prefix(name)?.strip_prefix(':')?)?)
                .ok()?;
        }
        if counts.next().is_some() || fields.next().is_some() {
            return None;
        }
        Some(Self {
            elapsed,
            period,
            first_frame,
            accepted,
            submitted,
            abandoned,
            skipped,
            longest_gap,
            detail,
            blocked,
        })
    }

    /// Write the relayed motion to the application log under the child's run.
    #[cfg(feature = "windows-native-host")]
    pub(crate) fn relay(self, run: sugarsubstitute_contracts::lifecycle::RunKey) {
        let ms = |value: u32| f64::from(value) / 1000.0;
        tracing::info!(operation = "desktop_child_canvas_motion", run = ?run,
            elapsed_ms = ms(self.elapsed), period_ms = ms(self.period),
            first_frame_ms = self.first_frame.map(ms), accepted = self.accepted,
            submitted = self.submitted, abandoned = self.abandoned, skipped = self.skipped,
            longest_gap_ms = ms(self.longest_gap), detail = self.detail,
            blocked = %Counts(self.blocked), "A camera motion settled");
    }
}

/// The blocked counts on the wire: `name:count` pairs joined by commas, in `GATES` order.
struct Counts([u16; 17]);

impl std::fmt::Display for Counts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, (name, count)) in GATES.iter().zip(self.0).enumerate() {
            if index > 0 {
                f.write_str(",")?;
            }
            write!(f, "{name}:{count}")?;
        }
        Ok(())
    }
}

/// An optional count on the wire: digits, or `none`.
struct Optional(Option<u32>);

impl std::fmt::Display for Optional {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => write!(f, "{value}"),
            None => f.write_str("none"),
        }
    }
}

/// Refuse signs, padding and overflow at the passive wire boundary.
#[cfg(feature = "windows-native-host")]
fn number(value: &str) -> Option<u32> {
    if value.is_empty()
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

#[cfg(all(test, feature = "windows-native-host"))]
#[path = "canvas_motion_diagnostic/tests.rs"]
mod tests;
