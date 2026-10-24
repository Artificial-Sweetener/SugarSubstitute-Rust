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

/// Observe cancellation and original deadline before interpreting returned native I/O.
#[cfg(test)]
pub(in crate::readiness_transport) fn complete(
    result: windows::core::Result<()>,
    stage: &'static str,
    peer_observed: bool,
    context: &JobContext,
) -> Result<(), ReadinessTransportError> {
    super::checkpoint(context)?;
    result.map_err(|cause| request_failure(stage, cause, peer_observed))
}

/// Preserve exact native retry semantics after shared transport's cancellation check.
pub(in crate::readiness_transport) fn transport(
    cause: sugarsubstitute_networking::Failure,
    peer_observed: bool,
    context: &JobContext,
) -> ReadinessTransportError {
    let (stage, cause) = cause.into_parts();
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
mod tests {
    use super::*;
    use crate::readiness_transport::ProbeRetryDisposition;
    use std::error::Error as _;

    #[test]
    fn only_enumerated_first_connection_errors_permit_retry_and_preserve_cause() {
        for stage in ["http_send", "http_receive"] {
            for code in [12029, 12030] {
                let result = request_failure(stage, HRESULT::from_win32(code).into(), false);
                assert_eq!(
                    result.retry_disposition(),
                    ProbeRetryDisposition::TransientConnectionUnavailable
                );
                assert_eq!(result.stage(), stage);
                assert_eq!(
                    result.source().and_then(|e| e.downcast_ref::<Error>()).map(Error::code),
                    Some(HRESULT::from_win32(code))
                );
                assert_eq!(
                    request_failure(stage, HRESULT::from_win32(code).into(), true)
                        .retry_disposition(),
                    ProbeRetryDisposition::Terminal
                );
            }
        }
    }

    #[test]
    fn native_phase_timeouts_preserve_cause_before_and_after_peer_observation() {
        for stage in ["http_send", "http_receive", "http_body_read"] {
            for peer in [false, true] {
                let result = request_failure(stage, HRESULT::from_win32(12002).into(), peer);
                assert_eq!(result.stage(), stage);
                assert_eq!(result.retry_disposition(), ProbeRetryDisposition::ProbeIntervalElapsed);
                assert_eq!(
                    result.source().and_then(|e| e.downcast_ref::<Error>()).map(Error::code),
                    Some(HRESULT::from_win32(12002))
                );
            }
        }
    }

    #[test]
    fn cancel_auth_shape_memory_and_unknown_native_errors_remain_terminal() {
        for stage in ["http_send", "http_receive", "http_body_read"] {
            for peer in [false, true] {
                for code in [12017, 12015, 12152, 8, 12032, 0, 12001, 12003, u32::MAX] {
                    assert_eq!(
                        request_failure(stage, HRESULT::from_win32(code).into(), peer)
                            .retry_disposition(),
                        ProbeRetryDisposition::Terminal
                    );
                }
            }
        }
    }

    #[test]
    fn unrelated_operations_and_body_connection_errors_cannot_gain_retry() {
        for stage in
            ["http_timeouts", "http_headers", "http_close", "unknown", "HTTP_SEND", "http_send "]
        {
            for code in [12002, 12029, 12030] {
                assert_eq!(
                    request_failure(stage, HRESULT::from_win32(code).into(), false)
                        .retry_disposition(),
                    ProbeRetryDisposition::Terminal
                );
            }
        }
        for code in [12029, 12030] {
            assert_eq!(
                request_failure("http_body_read", HRESULT::from_win32(code).into(), false)
                    .retry_disposition(),
                ProbeRetryDisposition::Terminal
            );
        }
    }
}
