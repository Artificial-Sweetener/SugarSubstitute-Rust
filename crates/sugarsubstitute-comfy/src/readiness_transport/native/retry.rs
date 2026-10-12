// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify only first-connection failures reported by WinHTTP send/receive.
//!
//! Winhttp.h defines CANNOT_CONNECT=12029 and CONNECTION_ERROR=12030. They
//! describe a failed connection or connection reset/termination. This fixed
//! literal HTTP transport performs no TLS negotiation. Other native failures,
//! including timeout and malformed response, remain terminal. Once any request
//! establishes trusted peer observations, later failures are terminal too.
//! See <https://learn.microsoft.com/en-us/windows/win32/winhttp/error-messages>.
use super::ReadinessTransportError;
use windows::core::{Error, HRESULT};

/// Preserve the native cause with its prevalidation connection disposition.
pub(super) fn request_failure(
    stage: &'static str,
    cause: Error,
    peer_observed: bool,
) -> ReadinessTransportError {
    if !peer_observed
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
        for code in [12029, 12030] {
            let result = request_failure("http_send", HRESULT::from_win32(code).into(), false);
            assert_eq!(
                result.retry_disposition(),
                ProbeRetryDisposition::TransientConnectionUnavailable
            );
            assert_eq!(
                result.source().and_then(|e| e.downcast_ref::<Error>()).map(Error::code),
                Some(HRESULT::from_win32(code))
            );
            assert_eq!(
                request_failure("http_receive", HRESULT::from_win32(code).into(), true)
                    .retry_disposition(),
                ProbeRetryDisposition::Terminal
            );
        }
    }

    #[test]
    fn timeout_cancel_auth_shape_and_memory_native_errors_remain_terminal() {
        for code in [12002, 12017, 12015, 12152, 8, 12032] {
            assert_eq!(
                request_failure("http_receive", HRESULT::from_win32(code).into(), false)
                    .retry_disposition(),
                ProbeRetryDisposition::Terminal
            );
        }
    }
}
