// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the real Windows PAC engine using isolated local scripts, never registry changes.
use super::*;
use crate::test_support::{Running, TestResult, run_reserved, run_reserved_expiring};
use sugarsubstitute_execution::http_fixture::Exchange;

fn exchange(path: &str, range: Option<&str>, status: u16, headers: &str, body: &[u8]) -> Exchange {
    let mut response =
        format!("HTTP/1.1 {status} Fixture\r\n{headers}Connection: close\r\n\r\n").into_bytes();
    response.extend_from_slice(body);
    Exchange {
        path: format!("/{path}"),
        range: range.map(str::to_owned),
        response,
        await_client_close: false,
        cancel_client: None,
    }
}

#[test]
fn windows_pac_preserves_proxy_order_and_direct_decisions() -> TestResult {
    run_reserved(8 * 1024 * 1024, |context, _| {
        let script = br#"function FindProxyForURL(url, host) { if (host === 'direct.invalid') return 'DIRECT'; return 'PROXY first.invalid:8123; HTTPS second.invalid:8443; DIRECT'; }"#;
        let (server, port) = Running::start(
            vec![exchange(
                "policy.pac",
                None,
                200,
                &format!(
                    "Content-Type: application/x-ns-proxy-autoconfig\r\nContent-Length: {}\r\n",
                    script.len()
                ),
                script,
            )],
            context,
        )?;
        let pac = format!("http://127.0.0.1:{port}/policy.pac");
        let mut resolver = Resolver::new();
        let routes = resolver.resolve_with("https://proxied.invalid/file", Some(&pac), context)?;
        assert_eq!(
            routes,
            vec![
                Route::Proxy { url: "http://first.invalid:8123/".into() },
                Route::Proxy { url: "https://second.invalid:8443/".into() },
                Route::Direct
            ]
        );
        assert!(resolver.settled());
        // WinHTTP may cache PAC bodies, but must evaluate the target URL on every invocation.
        assert_eq!(
            resolver.resolve_with("https://direct.invalid/file", Some(&pac), context)?,
            vec![Route::Direct]
        );
        assert!(resolver.settled());
        server.finish()
    })
}

#[test]
fn malformed_pac_fails_without_direct_fallback_and_retires() -> TestResult {
    run_reserved(8 * 1024 * 1024, |context, _| {
        let script = b"not a PAC program";
        let (server, port) = Running::start(
            vec![exchange(
                "broken.pac",
                None,
                200,
                &format!(
                    "Content-Type: application/x-ns-proxy-autoconfig\r\nContent-Length: {}\r\n",
                    script.len()
                ),
                script,
            )],
            context,
        )?;
        let pac = format!("http://127.0.0.1:{port}/broken.pac");
        let mut resolver = Resolver::new();
        assert!(resolver.resolve_with("https://target.invalid/file", Some(&pac), context).is_err());
        assert!(resolver.settled());
        server.finish()
    })
}

#[test]
fn invalid_destinations_and_expired_jobs_never_acquire_native_custody() -> TestResult {
    run_reserved_expiring(RESERVATION_BYTES, |context, clock| {
        let mut resolver = Resolver::new();
        for url in [
            "file:///test",
            "https://user:password@target.invalid/",
            "https://target.invalid/#fragment",
            "nonsense",
        ] {
            assert!(resolver.resolve(url, context).is_err());
            assert!(resolver.settled());
        }
        clock.expire();
        assert_eq!(
            resolver
                .resolve("https://target.invalid/", context)
                .err()
                .ok_or("Expected expiry")?
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(resolver.settled());
        Ok(())
    })
}

#[test]
fn expired_pending_native_lookup_observes_final_callback_before_releasing_custody() -> TestResult {
    run_reserved_expiring(RESERVATION_BYTES, |context, clock| {
        use std::{
            net::TcpListener,
            sync::{Arc, Mutex},
        };
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let pac = format!("http://127.0.0.1:{}/pending.pac", listener.local_addr()?.port());
        let accepted = Arc::new(Mutex::new(None));
        let observed = accepted.clone();
        // Expiry follows an actual OS connection to the deliberately unanswered PAC server.
        clock.observe_checks(move || {
            let mut connection = observed.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if connection.is_some() {
                return true;
            }
            match listener.accept() {
                Ok((socket, _)) => {
                    *connection = Some(Ok(socket));
                    true
                }
                Err(cause) if cause.kind() == io::ErrorKind::WouldBlock => false,
                Err(cause) => {
                    *connection = Some(Err(cause));
                    true
                }
            }
        })?;
        let mut resolver = Resolver::new();
        let failure = resolver
            .resolve_with("https://target.invalid/", Some(&pac), context)
            .err()
            .ok_or("Pending lookup unexpectedly completed")?;
        assert_eq!(failure.kind(), io::ErrorKind::TimedOut);
        let connection = accepted.lock().map_err(|_| "PAC observation poisoned")?;
        assert!(
            connection.as_ref().is_some_and(|value| value.is_ok()),
            "PAC connection was not observed"
        );
        assert!(resolver.settled());
        resolver.retire()?;
        assert!(resolver.settled());
        Ok(())
    })
}
