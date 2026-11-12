// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed observational probe facts carried by the existing authenticated stderr owner.
//! These schema-one records never grant readiness, native privileges or receipt authority.

/// Fixed failed checks in SAMPLE, ordered from bit zero through bit fourteen:
/// graph initialized, Vue ready, position conversion, public API, extension array,
/// named extension, exact extension identity, graph equality, connected canvas bounds,
/// complete document, graph configuration finished, splash absent, setup done, library API, public API still present after import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Blocked(u16);
impl Blocked {
    /// Reject empty masks and unknown conditions; this value only describes incompleteness.
    pub fn new(bits: u16) -> Option<Self> {
        (bits != 0 && bits <= 32767).then_some(Self(bits))
    }
    /// Borrow only the closed numeric mask, never frontend strings or exception payloads.
    pub fn bits(self) -> u16 {
        self.0
    }
}

/// A passive observation of actual evaluation custody on the creating thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    /// The first actual native evaluation submission succeeded.
    Submitted,
    /// An actual evaluation completed with fixed incomplete-condition bits.
    Incomplete,
    /// An actual evaluation supplied a validated receipt awaiting its native barrier.
    Receipt,
    /// The actual owned evaluation completion failed decoding or attribution.
    Fault,
    /// The original inclusive frontend cutoff expired.
    Timeout,
}

/// Numeric, run-bound output metadata; none of these fields supplies native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Record {
    /// Original launch identity; nonzero.
    pub launch: u64,
    /// Original navigation generation; nonzero.
    pub navigation: u64,
    /// Last actually submitted operation; zero only before the first submission.
    pub probe_operation: u64,
    /// Actual native submissions in this actor lifetime.
    pub issued: u64,
    /// Actual matching evaluation callbacks consumed in this actor lifetime.
    pub completed: u64,
    /// Fixed failed conditions of the last incomplete disposition, otherwise zero.
    pub blocked: u16,
    /// Closed observed phase; this is never a startup command.
    pub stage: Stage,
}
impl Record {
    /// Decode one complete schema-one frame under the existing 512-byte scratch bound.
    /// Reject unknown fields, noncanonical integers and inconsistent custody counters.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 512 {
            return None;
        }
        let text = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
        let (prefix, fields) = text.split_once("operation=\"frontend_probe_diagnostic\" ")?;
        if prefix.contains("operation=")
            || !prefix.ends_with("sugarsubstitute_desktop::frontend_probe_diagnostic: ")
        {
            return None;
        }
        let mut fields = fields.split_ascii_whitespace();
        if fields.next()? != "schema=1" {
            return None;
        }
        let stage = match fields.next()? {
            "stage=Submitted" => Stage::Submitted,
            "stage=Incomplete" => Stage::Incomplete,
            "stage=Receipt" => Stage::Receipt,
            "stage=Fault" => Stage::Fault,
            "stage=Timeout" => Stage::Timeout,
            _ => return None,
        };
        let launch = number(fields.next()?, "launch=")?;
        let navigation = number(fields.next()?, "navigation=")?;
        let probe_operation = number(fields.next()?, "probe_operation=")?;
        let issued = number(fields.next()?, "issued=")?;
        let completed = number(fields.next()?, "completed=")?;
        let blocked = u16::try_from(number(fields.next()?, "blocked=")?).ok()?;
        if fields.next().is_some()
            || launch == 0
            || navigation == 0
            || probe_operation != issued
            || completed > issued
            || issued - completed > 1
            || blocked > 32767
            || blocked != 0 && completed == 0
        {
            return None;
        }
        match stage {
            Stage::Submitted if issued != 1 || completed != 0 || blocked != 0 => return None,
            Stage::Incomplete if completed == 0 || completed != issued || blocked == 0 => {
                return None;
            }
            Stage::Receipt | Stage::Fault
                if completed == 0 || completed != issued || blocked != 0 =>
            {
                return None;
            }
            _ => {}
        }
        Some(Self { launch, navigation, probe_operation, issued, completed, blocked, stage })
    }
}

/// Emit no free text so the original bounded output decoder can accept the whole frame.
#[cfg(target_os = "windows")]
pub fn emit(record: Record) {
    if record.stage == Stage::Timeout {
        tracing::warn!(operation="frontend_probe_diagnostic",schema=1,stage=?record.stage,
            launch=record.launch,navigation=record.navigation,probe_operation=record.probe_operation,
            issued=record.issued,completed=record.completed,blocked=record.blocked);
    } else {
        tracing::info!(operation="frontend_probe_diagnostic",schema=1,stage=?record.stage,
            launch=record.launch,navigation=record.navigation,probe_operation=record.probe_operation,
            issued=record.issued,completed=record.completed,blocked=record.blocked);
    }
}

fn number(field: &str, prefix: &str) -> Option<u64> {
    let value = field.strip_prefix(prefix)?;
    if value.is_empty()
        || value.len() > 20
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

#[cfg(test)]
#[path = "frontend_probe_diagnostic_tests.rs"]
mod tests;
