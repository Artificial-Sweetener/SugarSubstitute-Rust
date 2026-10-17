// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own generic first-failure decoding and run-bound relay without exposing raw stderr.
use super::{Failure, console, consume, startup};
use sugarsubstitute_contracts::lifecycle::RunKey;

/// Accept only one exact marker and complete closed fields, rejecting extra raw payloads.
pub(super) fn parse(bytes: &[u8]) -> Option<Failure> {
    if bytes.len() > 512 {
        return None;
    }
    let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
    let (prefix, fields) = line.split_once("operation=\"desktop_failure_transition\" ")?;
    if prefix.contains("operation=") {
        return None;
    }
    let mut fields = fields.split_ascii_whitespace();
    let boundary = match fields.next()?.strip_prefix("boundary=")? {
        "PollFrame" => "PollFrame",
        "RequestFrame" => "RequestFrame",
        "ObserveStartup" => "ObserveStartup",
        "TakeCommand" => "TakeCommand",
        "Visibility" => "Visibility",
        "PreSurface" => "PreSurface",
        "Cleanup" => "Cleanup",
        "Presentation" => "Presentation",
        "CursorMonitor" => "CursorMonitor",
        "MonitorWorkArea" => "MonitorWorkArea",
        "CenterWindow" => "CenterWindow",
        "Other" => "Other",
        _ => return None,
    };
    let kind = match fields.next()?.strip_prefix("kind=")? {
        "TimedOut" => "TimedOut",
        "Disconnected" => "Disconnected",
        "Denied" => "Denied",
        "InvalidInput" => "InvalidInput",
        "Io" => "Io",
        "Other" => "Other",
        _ => return None,
    };
    let (startup, consume, console) = if fields.clone().next().is_some() {
        match boundary {
            "Other" => (None, None, Some(console::parse(fields)?)),
            "ObserveStartup" if kind == "Other" => (Some(startup::parse(fields)?), None, None),
            "PollFrame" if kind == "Other" => (None, Some(consume::parse(fields)?), None),
            _ => return None,
        }
    } else {
        (None, None, None)
    };
    Some(Failure { boundary, kind, startup, consume, console })
}

/// Relay only closed metadata; Run comes from the authenticated output owner.
/// Preserve the existing target and legacy field sequence across owner extraction.
pub(in super::super) fn relay(run: RunKey, failure: Failure) {
    if let Some(startup) = failure.startup {
        tracing::warn!(target: "sugarsubstitute::production::entry_activation::desktop_output",operation="desktop_child_failure",run=?run,
                                boundary=%failure.boundary,kind=%failure.kind,
                                observation=%startup.observation,close_stage=?startup.stage,
                                capture_snapshot=?startup.capture,code=%startup.code,
                                "Observed first closed owned desktop failure");
    } else if let Some(consume) = failure.consume {
        tracing::warn!(target: "sugarsubstitute::production::entry_activation::desktop_output",operation="desktop_child_failure",run=?run,
                                boundary=%failure.boundary,kind=%failure.kind,
                                consume_stage=?consume.stage,category=%consume.code,
                                "Observed first closed owned desktop failure");
    } else if let Some(console) = failure.console {
        tracing::warn!(target: "sugarsubstitute::production::entry_activation::desktop_output",
                                operation="desktop_child_failure",run=?run,
                                boundary=%failure.boundary,kind=%failure.kind,
                                console_operation=?console.operation,console_code=?console.code,
                                "Observed first closed owned desktop failure");
    } else {
        tracing::warn!(target: "sugarsubstitute::production::entry_activation::desktop_output",operation="desktop_child_failure",run=?run,
                                boundary=%failure.boundary,kind=%failure.kind,
                                "Observed first closed owned desktop failure");
    }
}
