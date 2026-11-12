// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve readiness retry contracts at the actual shared-transport error boundary.
use super::*;
use crate::readiness_transport::ProbeRetryDisposition;
use std::error::Error as _;
use sugarsubstitute_networking::Cause;
mod support;
use support::{State, TestResult, observe};

#[test]
fn shared_native_timeouts_retain_operation_and_original_hresult() -> TestResult {
    observe(State::Live, |context| {
        for stage in ["http_send", "http_receive", "http_body_read"] {
            for peer in [false, true] {
                let error = classify(
                    (stage, Cause::Native(HRESULT::from_win32(12002).into())),
                    peer,
                    context,
                );
                assert_eq!(error.stage(), stage);
                assert_eq!(error.retry_disposition(), ProbeRetryDisposition::ProbeIntervalElapsed);
                assert_eq!(
                    error.source().and_then(|cause| cause.downcast_ref::<Error>()).map(Error::code),
                    Some(HRESULT::from_win32(12002))
                );
            }
        }
        Ok(())
    })
}

#[test]
fn shared_connection_failures_cannot_gain_retry_after_native_peer_binding() -> TestResult {
    observe(State::Live, |context| {
        for stage in ["http_send", "http_receive"] {
            for code in [12029, 12030] {
                for peer in [false, true] {
                    let error = classify(
                        (stage, Cause::Native(HRESULT::from_win32(code).into())),
                        peer,
                        context,
                    );
                    assert_eq!(
                        error.retry_disposition(),
                        if peer {
                            ProbeRetryDisposition::Terminal
                        } else {
                            ProbeRetryDisposition::TransientConnectionUnavailable
                        }
                    );
                    assert_eq!(
                        error
                            .source()
                            .and_then(|cause| cause.downcast_ref::<Error>())
                            .map(Error::code),
                        Some(HRESULT::from_win32(code))
                    );
                }
            }
        }
        Ok(())
    })
}

#[test]
fn shared_io_failures_keep_their_original_kind_and_cause_without_native_retry() -> TestResult {
    observe(State::Live, |context| {
        for kind in [
            std::io::ErrorKind::InvalidData,
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::TimedOut,
            std::io::ErrorKind::Interrupted,
        ] {
            let error = classify(
                ("http_send", Cause::Io(std::io::Error::new(kind, "original-io-cause"))),
                false,
                context,
            );
            assert_eq!(error.retry_disposition(), ProbeRetryDisposition::Terminal);
            let original = error
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .ok_or("Original I/O cause lost")?;
            assert_eq!(original.kind(), kind);
            assert_eq!(original.to_string(), "original-io-cause");
        }
        Ok(())
    })
}

#[test]
fn shared_submillisecond_budget_is_elapsed_only_for_the_typed_io_failure() -> TestResult {
    observe(State::Live, |context| {
        let error = classify(
            (
                "http_submillisecond_deadline",
                Cause::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "original finite budget",
                )),
            ),
            true,
            context,
        );
        assert_eq!(error.retry_disposition(), ProbeRetryDisposition::ProbeIntervalElapsed);
        assert_eq!(
            error
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .map(std::io::Error::kind),
            Some(std::io::ErrorKind::TimedOut)
        );
        let native = classify(
            ("http_submillisecond_deadline", Cause::Native(HRESULT::from_win32(12002).into())),
            false,
            context,
        );
        assert_eq!(native.retry_disposition(), ProbeRetryDisposition::Terminal);
        Ok(())
    })
}

#[test]
fn actual_job_cancellation_precedes_native_connection_or_timeout_retry() -> TestResult {
    observe(State::Cancelled, |context| {
        assert!(context.cancellation_requested());
        for stage in ["http_send", "http_receive", "http_body_read", "http_checkpoint"] {
            for code in [12002, 12029, 12030] {
                let error = classify(
                    (stage, Cause::Native(HRESULT::from_win32(code).into())),
                    false,
                    context,
                );
                assert_eq!(error.stage(), "attempt_cancelled");
                assert_eq!(error.retry_disposition(), ProbeRetryDisposition::Terminal);
            }
        }
        Ok(())
    })
}

#[test]
fn original_job_expiration_prevents_late_native_results_from_gaining_retry() -> TestResult {
    observe(State::Expired, |context| {
        assert!(context.deadline().is_some_and(|deadline| context.now() >= deadline));
        for stage in ["http_send", "http_receive", "http_body_read", "http_checkpoint"] {
            let error =
                classify((stage, Cause::Native(HRESULT::from_win32(12029).into())), false, context);
            assert!(matches!(error.stage(), "attempt_deadline" | "attempt_cancelled"));
            assert_eq!(error.retry_disposition(), ProbeRetryDisposition::Terminal);
        }
        Ok(())
    })
}

#[test]
fn cancellation_cannot_replace_the_original_native_retirement_failure() -> TestResult {
    observe(State::Cancelled, |context| {
        let error = classify(
            ("http_close", Cause::Native(HRESULT::from_win32(12002).into())),
            false,
            context,
        );
        assert_eq!(error.stage(), "http_close");
        assert_eq!(error.retry_disposition(), ProbeRetryDisposition::Terminal);
        assert_eq!(
            error.source().and_then(|cause| cause.downcast_ref::<Error>()).map(Error::code),
            Some(HRESULT::from_win32(12002))
        );
        Ok(())
    })
}

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
                request_failure(stage, HRESULT::from_win32(code).into(), true).retry_disposition(),
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
                request_failure(stage, HRESULT::from_win32(code).into(), false).retry_disposition(),
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
