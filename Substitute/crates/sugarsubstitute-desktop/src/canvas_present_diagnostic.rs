// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed schema-two facts about what embedded canvas presentation cost over one window.
//!
//! The record counts the frames the composition presenter committed in about a second of
//! activity, how each frame's pixels reached Direct2D, how much of the retained surface each one
//! redrew, and where the time went. It carries numbers only and grants no capability. The owned
//! child writes it to its authenticated stderr, where the parent's bounded decoder relays it as
//! its own structured event.

/// One window of committed canvas frames, in whole microseconds, bytes, counts and pixels.
///
/// A window spans about a second, so its counts, microsecond sums and pixel sizes fit 32 bits
/// (sums saturate); only the bytes handed over need 64. The relay carries the record inline in
/// its observation, which this keeps within the size of the relay's original metadata.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Record {
    /// From the window's first frame to its last.
    pub window_us: u32,
    /// Frames committed.
    pub presents: u32,
    /// CPU frames that allocated a new device bitmap.
    pub created: u32,
    /// CPU frames that refilled the surface's retained bitmap.
    pub updated: u32,
    /// Shared GPU frames wrapped without copying.
    pub wrapped: u32,
    /// Frames shown by redrawing only their changed regions of the retained surface.
    pub repaired: u32,
    /// Repaired frames that first moved the surface's retained pixels.
    pub scrolled: u32,
    /// Repaired frames that changed no pixel.
    pub unchanged: u32,
    /// Regions the repaired frames redrew.
    pub regions: u32,
    /// CPU pixel bytes handed to Direct2D.
    pub source_bytes: u64,
    /// Opening surface updates and handing pixels over (for shared frames: wrapping the texture).
    pub fill_us: u32,
    /// Clearing, drawing and closing surface updates.
    pub draw_us: u32,
    /// Queuing the shared texture's read fence and flushing the device.
    pub signal_us: u32,
    /// Binding, placing and committing to composition.
    pub commit_us: u32,
    /// The slowest single frame, fill through commit.
    pub slowest_us: u32,
    /// The last frame's viewport width in physical pixels.
    pub width: u32,
    /// The last frame's viewport height in physical pixels.
    pub height: u32,
}

/// The exact producer target, which the parser requires before the closed fields.
const TARGET: &str = "sugarsubstitute_desktop::canvas_present_diagnostic: ";
/// The closed operation code shared by producer and parser.
const MARKER: &str = "operation=\"canvas_present\" ";
/// The relay's line bound; longer lines are never decoded.
const LINE_BYTES: usize = 512;
/// The fields after the schema, in the order producer and parser share.
const FIELDS: [&str; 17] = [
    "window_us",
    "presents",
    "created",
    "updated",
    "wrapped",
    "repaired",
    "scrolled",
    "unchanged",
    "regions",
    "source_bytes",
    "fill_us",
    "draw_us",
    "signal_us",
    "commit_us",
    "slowest_us",
    "width",
    "height",
];

impl Record {
    /// Emit the schema-two record at `info`, in the field order [`Record::parse`] requires.
    pub fn emit(self) {
        tracing::info!(
            operation = "canvas_present",
            schema = 2,
            window_us = self.window_us,
            presents = self.presents,
            created = self.created,
            updated = self.updated,
            wrapped = self.wrapped,
            repaired = self.repaired,
            scrolled = self.scrolled,
            unchanged = self.unchanged,
            regions = self.regions,
            source_bytes = self.source_bytes,
            fill_us = self.fill_us,
            draw_us = self.draw_us,
            signal_us = self.signal_us,
            commit_us = self.commit_us,
            slowest_us = self.slowest_us,
            width = self.width,
            height = self.height
        );
    }

    /// Decode one complete schema-two line from this producer under the 512-byte line bound.
    /// Unknown, missing, extra or noncanonical fields are rejected.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > LINE_BYTES {
            return None;
        }
        let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
        let (prefix, fields) = line.split_once(MARKER)?;
        if prefix.contains("operation=") || !prefix.ends_with(TARGET) {
            return None;
        }
        let mut fields = fields.split_ascii_whitespace();
        let mut field = |name: &str| fields.next()?.strip_prefix(name)?.strip_prefix('=');
        if field("schema")? != "2" {
            return None;
        }
        let mut values = [0_u64; 17];
        for (slot, name) in values.iter_mut().zip(FIELDS) {
            *slot = number(field(name)?)?;
        }
        if fields.next().is_some() {
            return None;
        }
        let [
            window_us,
            presents,
            created,
            updated,
            wrapped,
            repaired,
            scrolled,
            unchanged,
            regions,
            source_bytes,
            fill_us,
            draw_us,
            signal_us,
            commit_us,
            slowest_us,
            width,
            height,
        ] = values;
        let narrow = |value: u64| u32::try_from(value).ok();
        Some(Self {
            window_us: narrow(window_us)?,
            presents: narrow(presents)?,
            created: narrow(created)?,
            updated: narrow(updated)?,
            wrapped: narrow(wrapped)?,
            repaired: narrow(repaired)?,
            scrolled: narrow(scrolled)?,
            unchanged: narrow(unchanged)?,
            regions: narrow(regions)?,
            source_bytes,
            fill_us: narrow(fill_us)?,
            draw_us: narrow(draw_us)?,
            signal_us: narrow(signal_us)?,
            commit_us: narrow(commit_us)?,
            slowest_us: narrow(slowest_us)?,
            width: narrow(width)?,
            height: narrow(height)?,
        })
    }
}

/// Accept a canonical decimal `u64`, rejecting signs, padding and empty text.
fn number(raw: &str) -> Option<u64> {
    if raw.is_empty()
        || (raw.len() > 1 && raw.starts_with('0'))
        || !raw.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    raw.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = "2026-11-24T22:59:44.926536Z  INFO \
        sugarsubstitute_desktop::canvas_present_diagnostic: operation=\"canvas_present\" schema=2 \
        window_us=1000250 presents=60 created=1 updated=12 wrapped=47 repaired=9 scrolled=4 \
        unchanged=2 regions=14 source_bytes=431308800 \
        fill_us=41000 draw_us=900 signal_us=310 commit_us=5200 slowest_us=3100 \
        width=3840 height=2160";

    #[test]
    fn a_produced_line_decodes_to_its_exact_numbers() {
        assert_eq!(
            Record::parse(LINE.as_bytes()),
            Some(Record {
                window_us: 1_000_250,
                presents: 60,
                created: 1,
                updated: 12,
                wrapped: 47,
                repaired: 9,
                scrolled: 4,
                unchanged: 2,
                regions: 14,
                source_bytes: 431_308_800,
                fill_us: 41_000,
                draw_us: 900,
                signal_us: 310,
                commit_us: 5_200,
                slowest_us: 3_100,
                width: 3_840,
                height: 2_160,
            })
        );
        assert_eq!(
            Record::parse(format!("{LINE}\r").as_bytes()).map(|record| record.presents),
            Some(60)
        );
    }

    #[test]
    fn other_producers_and_altered_fields_are_rejected() {
        for line in [
            LINE.replace("sugarsubstitute_desktop::canvas_present_diagnostic", "page"),
            LINE.replace("schema=2", "schema=1"),
            LINE.replace("presents=60", "presents=060"),
            LINE.replace("presents=60", "presents=-1"),
            LINE.replace("presents=60", "presents=4294967296"),
            LINE.replace(" created=1", ""),
            LINE.replace(" signal_us=310", ""),
            format!("{LINE} extra=1"),
            format!("operation=\"spoof\" {LINE}"),
            format!("{LINE}{}", " ".repeat(LINE_BYTES)),
        ] {
            assert_eq!(Record::parse(line.as_bytes()), None, "{line}");
        }
    }
}
