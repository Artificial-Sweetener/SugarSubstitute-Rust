// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode compact main failure facts inside the authenticated desktop output lifetime.

/// Direct closed native/CDP error codes, without source text or native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum Code {
    Hresult(i32),
    Os(i32),
    Io(&'static str),
    Readiness(&'static str),
    JsonIo,
    JsonSyntax,
    JsonData,
    JsonEof,
    Opaque,
}

/// Inert launch/navigation facts bound to Run by the actual output owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct MainFailure {
    pub(in super::super) launch: u64,
    pub(in super::super) navigation: u64,
    pub(in super::super) operation: &'static str,
    pub(in super::super) observation: &'static str,
    pub(in super::super) code: Code,
    pub(in super::super) timeout: Option<Timeout>,
}

/// Only sealed stored readiness facts; zero is never an issued operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct Timeout {
    pub(in super::super) wait: &'static str,
    pub(in super::super) operation: Option<u64>,
}
const WAITS: &[&str] = &[
    "Fault",
    "Ready",
    "Failed",
    "Setup",
    "Navigation",
    "Root",
    "Context",
    "Document",
    "Probe",
    "Barrier",
];
const OPERATIONS: &[&str] = &[
    "frontend_budget_expired",
    "adopt_main_environment",
    "allocate_main_commands",
    "allocate_main_event_payload",
    "allocate_main_event_slots",
    "allocate_main_receivers",
    "allocate_readiness_contexts",
    "allocate_readiness_frames",
    "allocate_readiness_tables",
    "authorize_environment",
    "authorize_main_environment",
    "authorize_main_browser",
    "observe_main_environment",
    "begin_main_construction",
    "begin_native_navigation",
    "cancel_native_navigation",
    "claim_main_environment",
    "claim_main_events",
    "claim_main_native_metadata",
    "claim_main_receivers",
    "claim_readiness_contexts",
    "claim_readiness_frames",
    "claim_readiness_tables",
    "complete_main_command",
    "copy_main_observation",
    "create_main_controller",
    "create_main_window",
    "decode_main_observation",
    "deny_unowned_device_permission",
    "deny_unowned_download",
    "destroy_main_window",
    "drain_main_events",
    "enable_main_hooks",
    "encode_main_probe",
    "encode_main_versions",
    "focus_main_view",
    "get_main_event_payload",
    "get_main_receiver",
    "initialize_main_controls",
    "install_main_browser_events",
    "install_main_receiver",
    "install_main_receivers",
    "main_cdp_completion",
    "main_control",
    "main_event_overflow",
    "main_event_payload",
    "main_events",
    "main_frontend_effect",
    "main_frontend_evidence",
    "main_owned_callback",
    "main_readiness",
    "main_receiver_effect",
    "main_receivers",
    "main_request",
    "main_view",
    "main_window",
    "native_browser_failure",
    "native_navigation_completion",
    "native_navigation_input",
    "navigate_main_frontend",
    "observe_main_browser",
    "observe_main_visibility",
    "parse_main_receipt",
    "progress_main_construction",
    "query_main_environment",
    "remove_main_browser_events",
    "remove_main_receiver",
    "resize_main_view",
    "retire_main",
    "retire_main_constructor",
    "retire_main_view",
    "retire_partial_main",
    "reveal_main",
    "show_main_view",
    "submit_main_command",
    "take_main_result",
    "validate_main_identity",
    "validate_readiness_versions",
];
const OBSERVATIONS: &[&str] = &[
    "None",
    "Frame",
    "FrameDetached",
    "Context",
    "ContextDestroyed",
    "ContextsCleared",
    "Console",
    "NavigationStarting",
    "Navigation",
    "ProcessFailed",
    "Runtime",
    "Page",
    "Probe",
    "Barrier",
    "Command",
];

/// Require the exact compact fields; unknown operations/codes and raw extensions are discarded.
pub(super) fn parse(bytes: &[u8]) -> Option<MainFailure> {
    if bytes.len() > 512 {
        return None;
    }
    let line = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
    let (prefix, fields) = line.split_once("operation=\"main_failure_transition\" ")?;
    if prefix.contains("operation=") {
        return None;
    }
    let mut fields = fields.split_ascii_whitespace();
    let operation =
        known(fields.next()?.strip_prefix("failed_operation=\"")?.strip_suffix('"')?, OPERATIONS)?;
    let launch = identity(fields.next()?.strip_prefix("launch=")?)?;
    let navigation = identity(fields.next()?.strip_prefix("navigation=")?)?;
    let observation = known(fields.next()?.strip_prefix("observation=")?, OBSERVATIONS)?;
    let code = code(fields.next()?.strip_prefix("code=")?)?;
    let timeout = if operation == "frontend_budget_expired" {
        if observation != "None" || code != Code::Io("TimedOut") {
            return None;
        }
        let wait = known(fields.next()?.strip_prefix("wait=")?, WAITS)?;
        let value = fields.next()?.strip_prefix("probe_operation=")?;
        let issued = if value == "None" { None } else { Some(identity(value)?) };
        if matches!(wait, "Probe" | "Barrier") != issued.is_some() {
            return None;
        }
        Some(Timeout { wait, operation: issued })
    } else {
        None
    };
    if fields.next().is_some() {
        return None;
    }
    Some(MainFailure { launch, navigation, operation, observation, code, timeout })
}

/// Match borrowed input to a fixed authored token, retaining no raw input bytes.
fn known(value: &str, accepted: &'static [&'static str]) -> Option<&'static str> {
    accepted.iter().copied().find(|candidate| *candidate == value)
}
/// Require canonical nonzero decimal identities before carrying inert numeric facts.
fn identity(value: &str) -> Option<u64> {
    if value.starts_with('0') || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}
/// Decode signed direct native codes without allocating or accepting noncanonical forms.
fn native(value: &str) -> Option<i32> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits.starts_with('0') && digits.len() > 1)
        || value == "-0"
    {
        return None;
    }
    value.parse().ok()
}
/// Closed variant grammar prevents arbitrary formatted error payloads from reaching tracing.
fn code(value: &str) -> Option<Code> {
    match value {
        "Opaque" => Some(Code::Opaque),
        "JsonIo" => Some(Code::JsonIo),
        "JsonSyntax" => Some(Code::JsonSyntax),
        "JsonData" => Some(Code::JsonData),
        "JsonEof" => Some(Code::JsonEof),
        _ => {
            if let Some(value) = value.strip_prefix("Hresult(").and_then(|s| s.strip_suffix(')')) {
                return native(value).map(Code::Hresult);
            }
            if let Some(value) = value.strip_prefix("Os(").and_then(|s| s.strip_suffix(')')) {
                return native(value).map(Code::Os);
            }
            if let Some(value) = value.strip_prefix("Io(").and_then(|s| s.strip_suffix(')')) {
                return known(
                    value,
                    &["TimedOut", "Disconnected", "Denied", "InvalidInput", "Io", "Other"],
                )
                .map(Code::Io);
            }
            if let Some(value) = value.strip_prefix("Readiness(").and_then(|s| s.strip_suffix(')'))
            {
                return known(
                    value,
                    &[
                        "MalformedObservation",
                        "ObservationOverflow",
                        "WrongOrigin",
                        "ContextLost",
                        "NativeNavigationFailed",
                        "SetupFailed",
                        "DisabledExtension",
                        "ForeignReceipt",
                        "VersionMismatch",
                        "StaleProbe",
                        "ProbeBusy",
                        "MissingDomResult",
                        "IdentityExhausted",
                        "Cancelled",
                    ],
                )
                .map(Code::Readiness);
            }
            None
        }
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
