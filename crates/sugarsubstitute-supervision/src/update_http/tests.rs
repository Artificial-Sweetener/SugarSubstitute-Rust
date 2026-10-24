// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual WinHTTP requests against isolated execution-owned loopback sockets.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, installation};
mod signed;
use super::fixture::{Running, complete, exchange};
const IDLE: Duration = Duration::from_secs(1);
const RESERVATION: u64 = 24 * 1024 * 1024;

#[test]
fn silent_server_times_out_and_observes_the_client_close_its_socket() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let mut stalled = exchange("stall", None, 200, "Content-Length: 10\r\n", b"short");
        stalled.await_client_close = true;
        let (server, policy) = Running::start(vec![stalled], context)?;
        let mut http = Http::new();
        let result =
            http.metadata(&policy, policy.asset("stall")?, 100, Duration::from_millis(50), context);
        assert!(matches!(result, Err(Failure::Io(_))));
        assert!(http.settled());
        server.finish()
    })
}

#[test]
fn actual_http_follows_approved_redirect_and_reuses_settled_custody() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let (server, policy) = Running::start(
            vec![
                exchange(
                    "first",
                    None,
                    302,
                    "Location: /next?signature=fixture\r\nContent-Length: 0\r\nSet-Cookie: unwanted=secret\r\n",
                    b"",
                ),
                complete("next?signature=fixture", b"signed metadata"),
                complete("another", b"next"),
                exchange(
                    "chunked",
                    None,
                    200,
                    "Transfer-Encoding: chunked\r\n",
                    b"4\r\nnext\r\n0\r\n\r\n",
                ),
                exchange("missing", None, 404, "Content-Length: 0\r\n", b""),
            ],
            context,
        )?;
        let mut http = Http::new();
        assert_eq!(
            http.metadata(&policy, policy.asset("first")?, 100, IDLE, context)?,
            b"signed metadata"
        );
        assert!(http.settled());
        assert_eq!(http.metadata(&policy, policy.asset("another")?, 100, IDLE, context)?, b"next");
        assert!(http.settled());
        assert_eq!(http.metadata(&policy, policy.asset("chunked")?, 100, IDLE, context)?, b"next");
        assert!(http.settled());
        assert!(matches!(
            http.metadata(&policy, policy.asset("missing")?, 100, IDLE, context),
            Err(Failure::Policy(sugarsubstitute_update::http::Rejection::Status(404)))
        ));
        assert!(http.settled());
        server.finish()
    })
}

#[test]
fn forbidden_redirect_and_challenge_never_send_credentials_or_follow_foreign_origin() -> TestResult
{
    worker::run_reserved(RESERVATION, |context, _| {
        for (status, fields) in [
            (302, "Location: http://127.0.0.1:1/forbidden\r\n"),
            (401, "WWW-Authenticate: Negotiate\r\n"),
        ] {
            let (server, policy) = Running::start(
                vec![exchange(
                    "first",
                    None,
                    status,
                    &format!("{fields}Content-Length: 0\r\n"),
                    b"",
                )],
                context,
            )?;
            let mut http = Http::new();
            assert!(matches!(
                http.metadata(&policy, policy.asset("first")?, 100, IDLE, context),
                Err(Failure::Policy(_))
            ));
            assert!(http.settled());
            server.finish()?;
        }
        Ok(())
    })
}

#[test]
fn malformed_truncated_and_excess_metadata_retires_every_request() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        for (fields, body) in [
            ("Content-Length: 999\r\n", &b""[..]),
            ("Content-Length: 10\r\n", &b"short"[..]),
            ("Content-Encoding: gzip\r\nContent-Length: 4\r\n", &b"fake"[..]),
            ("Content-Length: 4\r\nContent-Length: 5\r\n", &b"wrong"[..]),
            ("", &b"body larger than bound"[..]),
        ] {
            let (server, policy) =
                Running::start(vec![exchange("bad", None, 200, fields, body)], context)?;
            let mut http = Http::new();
            assert!(
                http.metadata(&policy, policy.asset("bad")?, 10, IDLE, context).is_err(),
                "{fields}"
            );
            assert!(http.settled());
            server.finish()?;
        }
        Ok(())
    })
}

#[test]
fn redirect_loop_stops_at_bound_and_invalid_idle_opens_nothing() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let (server, policy) = Running::start(
            (0..6)
                .map(|_| {
                    exchange("loop", None, 302, "Location: /loop\r\nContent-Length: 0\r\n", b"")
                })
                .collect(),
            context,
        )?;
        let mut http = Http::new();
        for idle in [Duration::ZERO, Duration::from_secs(121)] {
            assert!(http.metadata(&policy, policy.asset("loop")?, 10, idle, context).is_err());
            assert!(http.settled());
        }
        assert!(http.metadata(&policy, policy.asset("loop")?, 10, IDLE, context).is_err());
        assert!(http.settled());
        server.finish()
    })
}
