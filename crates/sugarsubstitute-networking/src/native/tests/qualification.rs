// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Query actual native policy and qualify rejection/cancellation without contacting public hosts.
use super::super::*;
use crate::UpdateRequest;
use crate::test_support::{self as support, TestResult};
/// Fixed native option output remains inside this narrow adapter's qualification code.
fn option_value(handle: usize, key: u32) -> Result<u32, Failure> {
    let mut value = 0u32;
    let mut bytes = 4;
    // SAFETY: The live owned handle is borrowed; exactly one initialized DWORD is writable.
    unsafe {
        WinHttpQueryOption(raw(handle), key, Some((&mut value as *mut u32).cast()), &mut bytes)
    }
    .map_err(|e| {
        Failure::native(
            match key {
                WINHTTP_OPTION_ENABLE_HTTP_PROTOCOL => "qualify_http_protocol",
                WINHTTP_OPTION_CONNECT_RETRIES => "qualify_connect_retries",
                WINHTTP_OPTION_DISABLE_FEATURE => "qualify_disabled_features",
                WINHTTP_OPTION_REDIRECT_POLICY => "qualify_redirect_policy",
                WINHTTP_OPTION_MAX_RESPONSE_HEADER_SIZE => "qualify_header_limit",
                WINHTTP_OPTION_DECOMPRESSION => "qualify_decompression",
                WINHTTP_OPTION_AUTOLOGON_POLICY => "qualify_autologon",
                _ => "qualification_option",
            },
            e,
        )
    })?;
    assert_eq!(bytes, 4);
    Ok(value)
}
#[test]
fn actual_native_requests_preserve_redirect_header_tls_and_phase_timeout_controls() -> TestResult {
    support::run(false, 1024 * 1024, |context, _| {
        for update in [false, true] {
            let mut transport = Transport::default();
            let outcome = (|| -> TestResult {
                if update {
                    transport.update(
                        &UpdateRequest::https("example.invalid", 443, "/metadata")?,
                        100,
                        context,
                    )?;
                } else {
                    transport.readiness(1, context)?;
                    transport.readiness_request("/system_stats", context)?;
                }
                assert_eq!(option_value(transport.session, WINHTTP_OPTION_CONNECT_RETRIES)?, 1);
                assert_eq!(
                    option_value(transport.request, WINHTTP_OPTION_REDIRECT_POLICY)?,
                    WINHTTP_OPTION_REDIRECT_POLICY_NEVER
                );
                assert_eq!(
                    option_value(transport.request, WINHTTP_OPTION_MAX_RESPONSE_HEADER_SIZE)?,
                    16 * 1024
                );
                let security = option_value(transport.request, WINHTTP_OPTION_SECURITY_FLAGS)?;
                assert_eq!(
                    security
                        & (SECURITY_FLAG_IGNORE_CERT_CN_INVALID
                            | SECURITY_FLAG_IGNORE_CERT_DATE_INVALID
                            | SECURITY_FLAG_IGNORE_UNKNOWN_CA
                            | SECURITY_FLAG_IGNORE_CERT_WRONG_USAGE),
                    0
                );
                let mut url = [0u16; 512];
                let mut length = (url.len() * 2) as u32;
                // SAFETY: Complete fixed UTF-16 output is writable while the request remains owned.
                unsafe {
                    WinHttpQueryOption(
                        raw(transport.request),
                        WINHTTP_OPTION_URL,
                        Some(url.as_mut_ptr().cast()),
                        &mut length,
                    )
                }
                .map_err(|e| Failure::native("qualify_request_url", e))?;
                assert!(length as usize <= url.len() * 2 && length.is_multiple_of(2));
                let url = String::from_utf16(&url[..length as usize / 2])?;
                assert!(url.starts_with(if update {
                    "https://example.invalid/"
                } else {
                    "http://127.0.0.1:1/"
                }));
                for option in [
                    WINHTTP_OPTION_CONNECT_TIMEOUT,
                    WINHTTP_OPTION_SEND_TIMEOUT,
                    WINHTTP_OPTION_RECEIVE_TIMEOUT,
                    WINHTTP_OPTION_RESOLVE_TIMEOUT,
                ] {
                    let milliseconds = option_value(transport.request, option)?;
                    if update {
                        assert_eq!(milliseconds, 100);
                    } else {
                        assert!(milliseconds > 100 && milliseconds <= 10_000);
                    }
                }
                Ok(())
            })();
            let retirement = transport.close();
            outcome?;
            retirement?;
            assert!(transport.empty());
        }
        Ok(())
    })
}
#[test]
fn invalid_idle_and_cross_route_requests_reject_before_native_transfer() -> TestResult {
    support::run(false, 1024 * 1024, |context, _| {
        let request = UpdateRequest::https("example.invalid", 443, "/metadata")?;
        let mut transport = Transport::default();
        for idle in [0, 120_001, u32::MAX] {
            assert!(transport.update(&request, idle, context).is_err());
            assert!(transport.empty());
        }
        transport.update(&request, 100, context)?;
        transport.close_request()?;
        assert!(transport.readiness_request("/system_stats", context).is_err());
        assert!(transport.send_readiness(context).is_err());
        transport.close()?;
        assert!(transport.empty());
        transport.readiness(1, context)?;
        for path in ["http://example.invalid/", "//example.invalid/", "/x\r\nInjected: yes", ""] {
            assert!(transport.readiness_request(path, context).is_err());
        }
        assert!(transport.send_update(None, context).is_err());
        transport.close()?;
        assert!(transport.empty());
        Ok(())
    })
}
#[test]
fn cancellation_rejects_transfer_but_checked_native_retirement_remains_available() -> TestResult {
    support::run(true, 1024 * 1024, |context, scope| {
        let mut transport = Transport::default();
        transport.readiness(1, context)?;
        transport.readiness_request("/system_stats", context)?;
        scope.cancel();
        assert!(transport.send_readiness(context).is_err());
        assert!(!transport.empty());
        transport.close()?;
        assert!(transport.empty());
        Ok(())
    })
}

#[test]
fn actual_inflight_cancellation_retains_custody_until_the_peer_observes_native_close() -> TestResult
{
    support::run(true, 6 * 1024 * 1024, |context, scope| {
        let (peer, port) = support::Running::cancelling(context, scope)?;
        let request = UpdateRequest::loopback(port, "/cancel")?;
        let mut transport = Transport::default();
        let outcome = (|| -> Result<(), Failure> {
            transport.update(&request, 1000, context)?;
            transport.send_update(None, context)?;
            transport.receive(context)?;
            let mut bytes = [0; 32];
            loop {
                if transport.read(&mut bytes, context)? == 0 {
                    return Ok(());
                }
            }
        })();
        let retirement = transport.close();
        let cause =
            std::io::Error::from(outcome.err().ok_or("Cancelled request unexpectedly completed")?);
        assert_eq!(cause.kind(), std::io::ErrorKind::Interrupted);
        retirement?;
        assert!(transport.empty());
        peer.finish()
    })
}

#[test]
fn active_native_requests_reject_foreign_admitted_jobs_but_allow_checked_cleanup() -> TestResult {
    support::run(false, 1024 * 1024, |context, _| {
        let mut transport = Transport::default();
        transport.readiness(1, context)?;
        transport.readiness_request("/system_stats", context)?;
        support::run(false, 1024 * 1024, move |foreign, _| {
            let failure = transport
                .send_readiness(foreign)
                .err()
                .ok_or("Foreign job borrowed native request")?;
            assert_eq!(failure.into_parts().0, "http_context");
            assert!(!transport.empty());
            transport.close()?;
            assert!(transport.empty());
            Ok(())
        })
    })
}
