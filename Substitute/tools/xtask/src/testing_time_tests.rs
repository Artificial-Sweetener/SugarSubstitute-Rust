// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove the real-time rule rejects every known form and admits only exact registrations.

use super::*;
use crate::fixtures::{Repository, write};

const HEADER: &str = "// SPDX-License-Identifier: GPL-3.0-or-later\n";

/// Write `files` into a fixture host and check its test scope against `registered`.
fn checked(files: &[(&str, &str)], registered: &[RealTime]) -> Result<Repository> {
    let fixture = Repository::new()?;
    for (relative, contents) in files {
        write(fixture.root(), relative, &format!("{HEADER}{contents}"))?;
    }
    check(fixture.root(), registered, &fixture.architecture)?;
    Ok(fixture)
}

/// Whether the check fails with the real-time diagnostic, not an incidental parse or path error.
fn rejected(files: &[(&str, &str)]) -> bool {
    checked(files, &[])
        .err()
        .is_some_and(|failure| failure.to_string().contains("let real time decide"))
}

/// The fixed-timeout condvar wait the import cancellation test used is rejected.
#[test]
fn a_fixed_timeout_condvar_wait_in_a_test_module_is_rejected() {
    assert!(rejected(&[(
        "crates/host/src/lib.rs",
        "#[cfg(test)] mod tests { fn wait() { let (state, timeout) = wake\
         .wait_timeout_while(state, Duration::from_secs(20), |state| !state.release)\
         .map_err(|_| \"poisoned\")?; if timeout.timed_out() { return Err(\"late\".into()); } } }",
    )]));
}

/// An aliased Instant, a grouped self import and a fn-pointer reference are all resolved.
#[test]
fn aliased_and_referenced_clock_sources_are_rejected() {
    for body in [
        "use std::time::Instant as Clock; fn f() { let _ = Clock::now(); }",
        "use std::time::{self as clock}; fn f() { let _ = clock::Instant::now(); }",
        "fn f() { let anchor = std::sync::OnceLock::new(); anchor.get_or_init(Instant::now); }",
        "fn f() { let _ = std::time::SystemTime::now(); }",
        "fn f() { let _ = MonotonicClock::new(); }",
        "fn f(started: Instant) { assert!(started.elapsed() < LIMIT); }",
        "fn f(thread: Thread) { std::thread::park_timeout(D); }",
    ] {
        assert!(
            rejected(&[(
                "crates/host/src/lib.rs",
                &format!("#[cfg(test)] mod tests {{ {body} }}")
            )]),
            "{body}"
        );
    }
}

/// A stored timestamp compared with a fresh wall-clock read fails across a second boundary, so
/// both `unix_now()` and `SystemTime::now()` are rejected in that shape, including in macros.
#[test]
fn a_stored_timestamp_compared_with_a_fresh_wall_clock_read_is_rejected() {
    for body in [
        "fn f() -> TestResult { let record = authorize(&store)?; \
         assert_eq!(record.authorized_at, unix_now()?); Ok(()) }",
        "use crate::run::settlement::unix_now as now; \
         fn f(record: Record) { assert!(record.recovered_at <= now().unwrap()); }",
        "fn f(record: Record) { assert_eq!(record.at, std::time::SystemTime::now()\
         .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)); }",
        "fn f(record: Record) { assert_eq!(record.at, unix_now!()); }",
    ] {
        assert!(
            rejected(&[(
                "crates/host/src/lib.rs",
                &format!("#[cfg(test)] mod tests {{ {body} }}")
            )]),
            "{body}"
        );
    }
}
/// Timed waits hidden in macro arguments and macro definitions are rejected.
#[test]
fn macro_wrapped_timed_waits_are_rejected() {
    for body in [
        "fn f() { wrap!(events.recv_timeout(D)); }",
        "macro_rules! wait { ($events:expr) => { $events.recv_timeout(D) }; }",
        "fn f() { call!(events, recv_timeout, D); }",
        "fn f() { let _ = serde_json::json!({\"ms\": started.elapsed().as_millis()}); }",
    ] {
        assert!(
            rejected(&[(
                "crates/host/src/lib.rs",
                &format!("#[cfg(test)] mod tests {{ {body} }}")
            )]),
            "{body}"
        );
    }
}

/// Test scope follows the module tree, `tests/` and `*_fixture` campaigns, not file names.
#[test]
fn scope_follows_test_modules_fixtures_and_integration_tests() -> Result<()> {
    let real = "fn f() { let _ = std::time::Instant::now(); }";
    assert!(rejected(&[
        ("crates/host/src/lib.rs", "#[cfg(test)] mod renamed;"),
        ("crates/host/src/renamed.rs", real),
    ]));
    assert!(rejected(&[
        ("crates/host/src/lib.rs", "#[cfg(test)] #[path = \"elsewhere/x.rs\"] mod tests;"),
        ("crates/host/src/elsewhere/x.rs", real),
    ]));
    assert!(rejected(&[
        ("crates/host/src/lib.rs", "mod campaign_fixture;"),
        ("crates/host/src/campaign_fixture.rs", "mod parent;"),
        ("crates/host/src/campaign_fixture/parent.rs", real),
    ]));
    assert!(rejected(&[("crates/host/src/lib.rs", ""), ("crates/host/tests/contract.rs", real)]));
    assert!(rejected(&[(
        "crates/host/src/lib.rs",
        "struct S; impl S { #[cfg(test)] fn probe(&self) { let _ = Instant::now(); } }",
    )]));
    assert!(
        checked(&[("crates/host/src/lib.rs", "#[cfg(test)] mod missing;")], &[])
            .err()
            .is_some_and(|failure| failure.to_string().contains("has no source file"))
    );
    // Production code, non-test cfg branches and plain `elapsed` fields stay out of the rule.
    checked(
        &[(
            "crates/host/src/lib.rs",
            "fn product() -> std::time::Instant { std::time::Instant::now() }\n\
             #[cfg(any(test, feature = \"probe\"))] fn probe() { let _ = Instant::now(); }\n\
             #[cfg(test)] mod tests { fn f(report: Report) { assert_eq!(report.elapsed, D); \
             tracing::info!(elapsed = ?report.elapsed); } }",
        )],
        &[],
    )?;
    Ok(())
}

/// A registered guard passes only with its exact fingerprint, a real reason and actual use.
#[test]
fn registrations_admit_only_exact_current_used_files() -> Result<()> {
    let files = [
        ("crates/host/src/lib.rs", "#[cfg(test)] mod tests;"),
        (
            "crates/host/src/tests.rs",
            "fn f(started: Instant) { println!(\"{:?}\", started.elapsed()); }",
        ),
    ];
    let reason = "The evidence printer reports a measured duration only in a failure message and \
                  no assertion reads it.";
    let fixture = Repository::new()?;
    for (relative, contents) in files {
        write(fixture.root(), relative, &format!("{HEADER}{contents}"))?;
    }
    let path = "crates/host/src/tests.rs".to_owned();
    let current = fingerprint(fixture.root(), std::slice::from_ref(&path))?;
    let admitted = |source_sha256: &str, reason: &str| RealTime {
        path: path.clone(),
        source_sha256: source_sha256.into(),
        reason: reason.into(),
    };
    let refused = |registered: &[RealTime], message: &str| {
        check(fixture.root(), registered, &fixture.architecture)
            .err()
            .is_some_and(|failure| failure.to_string().contains(message))
    };
    check(fixture.root(), &[admitted(&current, reason)], &fixture.architecture)?;
    assert!(refused(&[admitted("sha256:stale", reason)], "stale real-time fingerprint"));
    assert!(refused(&[admitted(&current, "reviewed")], "file-specific real-time reason"));
    assert!(refused(&[admitted(&current, reason), admitted(&current, reason)], "twice"));
    assert!(refused(&[], "let real time decide"));
    write(fixture.root(), "crates/host/src/tests.rs", &format!("{HEADER}fn f() {{}}"))?;
    let unused = fingerprint(fixture.root(), std::slice::from_ref(&path))?;
    assert!(refused(&[admitted(&unused, reason)], "its test scope uses none"));
    Ok(())
}
