// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify exact WinHTTP request failures after original cancellation/deadline checks.
//!
//! TIMEOUT=12002 permits admitted readiness retry after checked cleanup at send, receive
//! and body read, including after a trusted peer observation. CANNOT_CONNECT=12029
//! and CONNECTION_ERROR=12030 permit only pre-peer send/receive connection retry.
//! Other stages and native failures remain terminal. No native cause is formatted.
//! See <https://learn.microsoft.com/en-us/windows/win32/winhttp/error-messages>.
use super::ReadinessTransportError;
use sugarsubstitute_contracts::execution::JobContext;
use windows::core::{Error, HRESULT};

/// Apply readiness retry policy to the shared transport's original cause.
/// Active-transfer cancellation and expiry take precedence; retirement retains native failures.
pub(in crate::readiness_transport) fn classify(
    (stage, cause): (&'static str, sugarsubstitute_networking::Cause),
    peer_observed: bool,
    context: &JobContext,
) -> ReadinessTransportError {
    if matches!(stage, "http_send" | "http_receive" | "http_body_read" | "http_checkpoint")
        && let Err(cause) = super::checkpoint(context)
    {
        return cause;
    }
    match cause {
        sugarsubstitute_networking::Cause::Native(cause) => {
            request_failure(stage, cause, peer_observed)
        }
        sugarsubstitute_networking::Cause::Io(cause) if stage == "http_submillisecond_deadline" => {
            ReadinessTransportError::elapsed(stage, cause)
        }
        sugarsubstitute_networking::Cause::Io(cause) => ReadinessTransportError::new(stage, cause),
    }
}

/// Preserve the original native cause under a closed operation/code classification.
fn request_failure(
    stage: &'static str,
    cause: Error,
    peer_observed: bool,
) -> ReadinessTransportError {
    if matches!(stage, "http_send" | "http_receive" | "http_body_read")
        && cause.code() == HRESULT::from_win32(12002)
    {
        ReadinessTransportError::elapsed(stage, cause)
    } else if matches!(stage, "http_send" | "http_receive")
        && !peer_observed
        && [12029, 12030].into_iter().any(|code| cause.code() == HRESULT::from_win32(code))
    {
        ReadinessTransportError::unavailable(stage, cause)
    } else {
        ReadinessTransportError::new(stage, cause)
    }
}

#[cfg(test)]
mod tests;
