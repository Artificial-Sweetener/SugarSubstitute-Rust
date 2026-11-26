// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed schema-one facts about one native GPU phase over one window of activity.
//!
//! The record names one phase of the selected device's work, how often it completed in about a
//! second and how long its calls took in wall time, which is not GPU execution time. It carries
//! numbers and one fixed phase name only and grants no capability. An owned child writes it to
//! its authenticated stderr, where the parent's bounded decoder relays it as its own event.

/// One phase window of one selected device, in whole microseconds, bytes and counts.
///
/// A window spans about a second, so its counts and microsecond sums fit 32 bits (sums saturate);
/// only bytes need 64. The relay carries the record inline in its observation, which this keeps
/// within the size of the relay's original metadata.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Record {
    /// The phase's fixed name, one of the device's closed phase inventory.
    pub phase: &'static str,
    /// The adapter's PCI vendor identifier.
    pub vendor: u32,
    /// The adapter's PCI device identifier.
    pub device: u32,
    /// Since the device's previous report.
    pub window_us: u32,
    /// Calls that returned in the window.
    pub completed: u32,
    /// Wall time of the calls that returned.
    pub total_us: u32,
    /// The longest call that returned.
    pub max_us: u32,
    /// Bytes the returned calls moved.
    pub bytes: u64,
    /// Work items the returned calls carried.
    pub work_items: u32,
    /// Native polls that timed out before completion.
    pub wait_timeouts: u32,
    /// Calls still outstanding at the report.
    pub active: u32,
    /// The most calls outstanding at once in the window.
    pub peak_active: u32,
    /// How long the outstanding calls have been continuously busy.
    pub busy_us: u32,
}

/// The exact producer target, which the parser requires before the closed fields.
const TARGET: &str = "sugarsubstitute_image_gpu::timing_diagnostic: ";
/// The closed operation code shared by producer and parser.
const MARKER: &str = "operation=\"image_gpu_timing\" ";
/// The relay's line bound; longer lines are never decoded.
const LINE_BYTES: usize = 512;
/// The numeric fields after the phase name, in the order producer and parser share.
const FIELDS: [&str; 12] = [
    "vendor",
    "device",
    "window_us",
    "completed",
    "total_us",
    "max_us",
    "bytes",
    "work_items",
    "wait_timeouts",
    "active",
    "peak_active",
    "busy_us",
];

impl Record {
    /// Emit the schema-one record at `info`, in the field order [`Record::parse`] requires.
    pub fn emit(self) {
        tracing::info!(
            operation = "image_gpu_timing",
            schema = 1,
            phase = self.phase,
            vendor = self.vendor,
            device = self.device,
            window_us = self.window_us,
            completed = self.completed,
            total_us = self.total_us,
            max_us = self.max_us,
            bytes = self.bytes,
            work_items = self.work_items,
            wait_timeouts = self.wait_timeouts,
            active = self.active,
            peak_active = self.peak_active,
            busy_us = self.busy_us
        );
    }

    /// Decode one complete schema-one line from this producer under the 512-byte line bound.
    /// Unknown phases and unknown, missing, extra or noncanonical fields are rejected.
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
        if field("schema")? != "1" {
            return None;
        }
        let named = field("phase")?.strip_prefix('"')?.strip_suffix('"')?;
        let phase = crate::timing::Phase::named(named)?;
        let mut values = [0_u64; 12];
        for (slot, name) in values.iter_mut().zip(FIELDS) {
            *slot = number(field(name)?)?;
        }
        if fields.next().is_some() {
            return None;
        }
        let [
            vendor,
            device,
            window_us,
            completed,
            total_us,
            max_us,
            bytes,
            work_items,
            wait_timeouts,
            active,
            peak_active,
            busy_us,
        ] = values;
        let narrow = |value: u64| u32::try_from(value).ok();
        Some(Self {
            phase,
            vendor: narrow(vendor)?,
            device: narrow(device)?,
            window_us: narrow(window_us)?,
            completed: narrow(completed)?,
            total_us: narrow(total_us)?,
            max_us: narrow(max_us)?,
            bytes,
            work_items: narrow(work_items)?,
            wait_timeouts: narrow(wait_timeouts)?,
            active: narrow(active)?,
            peak_active: narrow(peak_active)?,
            busy_us: narrow(busy_us)?,
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

    const LINE: &str = "2026-11-25T10:00:00.000000Z  INFO \
        sugarsubstitute_image_gpu::timing_diagnostic: operation=\"image_gpu_timing\" schema=1 \
        phase=\"display_draw\" vendor=4318 device=11141 window_us=1000400 completed=58 \
        total_us=41200 max_us=2100 bytes=0 work_items=58 wait_timeouts=0 active=0 \
        peak_active=1 busy_us=0";

    #[test]
    fn a_produced_line_decodes_to_its_exact_phase_and_numbers() {
        assert_eq!(
            Record::parse(LINE.as_bytes()),
            Some(Record {
                phase: "display_draw",
                vendor: 4318,
                device: 11141,
                window_us: 1_000_400,
                completed: 58,
                total_us: 41_200,
                max_us: 2_100,
                bytes: 0,
                work_items: 58,
                wait_timeouts: 0,
                active: 0,
                peak_active: 1,
                busy_us: 0,
            })
        );
    }

    #[test]
    fn other_producers_phases_and_altered_fields_are_rejected() {
        for line in [
            LINE.replace("sugarsubstitute_image_gpu::timing_diagnostic", "page"),
            LINE.replace("schema=1", "schema=2"),
            LINE.replace("display_draw", "spoofed_phase"),
            LINE.replace("phase=\"display_draw\"", "phase=display_draw"),
            LINE.replace("vendor=4318", "vendor=4294967296"),
            LINE.replace("completed=58", "completed=058"),
            LINE.replace("completed=58", "completed=4294967296"),
            LINE.replace(" bytes=0", ""),
            format!("{LINE} extra=1"),
            format!("operation=\"spoof\" {LINE}"),
            format!("{LINE}{}", " ".repeat(LINE_BYTES)),
        ] {
            assert_eq!(Record::parse(line.as_bytes()), None, "{line}");
        }
    }
}
