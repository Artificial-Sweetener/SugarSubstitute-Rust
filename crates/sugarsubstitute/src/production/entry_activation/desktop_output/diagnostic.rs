// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode only complete closed preparation fields, discarding every other raw line.
use sugarsubstitute_contracts::transcript::OutputStream;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Phase {
    pub(super) capture_admitted: bool,
    pub(super) capture: bool,
    pub(super) registered: bool,
    pub(super) configuration: bool,
    pub(super) original_failure: Option<&'static str>,
    pub(super) capture_failure: &'static str,
    pub(super) failure_retained: bool,
}
/// Fixed source scratch is covered by the containing Record's original metadata
/// claim. Loss, oversized input and EOF invalidate partial lines; stdout is opaque.
pub(super) struct Diagnostic {
    bytes: [u8; 512],
    length: usize,
    discard: bool,
    sequences: [u64; 2],
    latest: Option<Phase>,
    emitted: u8,
}
impl Diagnostic {
    pub(super) const fn new() -> Self {
        Self {
            bytes: [0; 512],
            length: 0,
            discard: false,
            sequences: [0; 2],
            latest: None,
            emitted: 0,
        }
    }
    /// The authenticated output owner validates Run before these inert byte facts.
    pub(super) fn feed(
        &mut self,
        stream: OutputStream,
        sequence: u64,
        gap: bool,
        end: bool,
        bytes: &[u8],
        mut emit: impl FnMut(Phase),
    ) {
        let at = stream.index();
        let discontinuity = gap || self.sequences[at].checked_add(1) != Some(sequence);
        if sequence <= self.sequences[at] {
            return;
        }
        self.sequences[at] = sequence;
        if stream != OutputStream::Stderr {
            return;
        }
        if discontinuity {
            self.length = 0;
            self.discard = true;
        }
        for &byte in bytes {
            if byte == b'\n' {
                if !self.discard
                    && let Some(phase) = parse(&self.bytes[..self.length])
                    && self.latest != Some(phase)
                {
                    self.latest = Some(phase);
                    if self.emitted < 32 {
                        self.emitted += 1;
                        emit(phase);
                    }
                }
                self.length = 0;
                self.discard = false;
            } else if !self.discard {
                if self.length == self.bytes.len() {
                    self.length = 0;
                    self.discard = true;
                } else {
                    self.bytes[self.length] = byte;
                    self.length += 1;
                }
            }
        }
        if end {
            self.length = 0;
            self.discard = false;
        }
    }
}
fn parse(bytes: &[u8]) -> Option<Phase> {
    let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
    let marker = "operation=\"child_preparation_transition\" ";
    let (prefix, fields) = line.split_once(marker)?;
    if prefix.contains("operation=") {
        return None;
    }
    let mut fields = fields.split_ascii_whitespace();
    let capture_admitted = boolean(fields.next()?, "capture_admitted=")?;
    let capture = boolean(fields.next()?, "capture=")?;
    let registered = boolean(fields.next()?, "registered=")?;
    let configuration = boolean(fields.next()?, "configuration=")?;
    let original_failure = match fields.next()?.strip_prefix("original_failure=")? {
        "None" => None,
        "Some(State)" => Some("State"),
        "Some(Identity)" => Some("Identity"),
        "Some(Transport)" => Some("Transport"),
        "Some(Capture)" => Some("Capture"),
        "Some(Configuration)" => Some("Configuration"),
        "Some(Deadline)" => Some("Deadline"),
        _ => return None,
    };
    let capture_failure = match fields.next()?.strip_prefix("capture_failure=")? {
        "None" => "None",
        "State" => "State",
        "Identity" => "Identity",
        "Admission" => "Admission",
        "Native" => "Native",
        "Data" => "Data",
        "Source" => "Source",
        "TimedOut" => "TimedOut",
        "Cancelled" => "Cancelled",
        "Uncertain" => "Uncertain",
        "Other" => "Other",
        _ => return None,
    };
    let failure_retained = boolean(fields.next()?, "failure_retained=")?;
    if fields.next().is_some() {
        return None;
    }
    Some(Phase {
        capture_admitted,
        capture,
        registered,
        configuration,
        original_failure,
        capture_failure,
        failure_retained,
    })
}
fn boolean(field: &str, prefix: &str) -> Option<bool> {
    match field.strip_prefix(prefix)? {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
