// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Tests run side by side on scoped threads only in the test files registered for it.

use crate::{
    error::Result,
    execution::{Policy, TestScheduling, check},
    fixtures::{Repository, write},
};

const LIBRARY: &str = "crates/host/src/lib.rs";
const TESTS: &str = "crates/host/src/tests.rs";
const FAN_OUT: &str = "fn cases() { std::thread::scope(|scope| { scope.spawn(|| {}); }); }";

/// A host crate whose test module is `tests`.
fn host(tests: &str) -> Result<Repository> {
    let fixture = Repository::new()?;
    write(fixture.root(), LIBRARY, "//! Fixture host.\n#[cfg(test)]\nmod tests;\n")?;
    write(fixture.root(), TESTS, tests)?;
    Ok(fixture)
}

fn registering(paths: &[&str], runs: &str) -> Policy {
    Policy {
        schema_version: 1,
        owner: None,
        test_scheduling: paths
            .iter()
            .map(|path| TestScheduling { path: (*path).into(), runs: runs.into() })
            .collect(),
    }
}

/// What the gate says of `fixture` under `policy`; empty when it passes.
fn verdict(fixture: &Repository, policy: &Policy) -> String {
    check(fixture.root(), policy, &fixture.architecture, &fixture.workspace)
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default()
}

#[test]
fn a_registered_test_file_may_fan_out_on_scoped_threads() -> Result<()> {
    let fixture = host(FAN_OUT)?;
    assert_eq!(verdict(&fixture, &registering(&[TESTS], "Independent cases.")), "");
    Ok(())
}

#[test]
fn an_unregistered_test_file_may_not_schedule() -> Result<()> {
    let fixture = host(FAN_OUT)?;
    let verdict = verdict(&fixture, &registering(&[], "Independent cases."));
    assert!(verdict.contains("outside the execution owner"), "unexpected result: {verdict}");
    Ok(())
}

#[test]
fn a_registered_file_may_use_scoped_threads_and_nothing_else() -> Result<()> {
    for tests in [
        "fn cases() { std::thread::scope(|_| {}); std::thread::spawn(|| {}); }",
        "fn cases() { std::thread::spawn(|| {}); }",
        "fn cases() { let _channel = std::sync::mpsc::channel::<u8>(); std::thread::scope(|_| {}); }",
        "fn cases() { std::thread::scope(|_| {}); rayon::scope(|_| {}); }",
    ] {
        let fixture = host(tests)?;
        let verdict = verdict(&fixture, &registering(&[TESTS], "Independent cases."));
        assert!(verdict.contains("may use only std::thread::scope"), "{tests}: {verdict}");
    }
    Ok(())
}

#[test]
fn application_code_cannot_be_registered() -> Result<()> {
    let fixture = Repository::new()?;
    write(
        fixture.root(),
        LIBRARY,
        "//! Fixture host.\npub fn work() { std::thread::scope(|_| {}); }\n",
    )?;
    let verdict = verdict(&fixture, &registering(&[LIBRARY], "Independent cases."));
    assert!(verdict.contains("outside test-only code"), "unexpected result: {verdict}");
    Ok(())
}

#[test]
fn a_registration_that_no_longer_schedules_is_removed() -> Result<()> {
    let fixture = host("fn cases() {}")?;
    let verdict = verdict(&fixture, &registering(&[TESTS], "Independent cases."));
    assert!(verdict.contains("remove its registration"), "unexpected result: {verdict}");
    Ok(())
}

#[test]
fn a_registration_says_what_it_runs_side_by_side() -> Result<()> {
    let fixture = host(FAN_OUT)?;
    let verdict = verdict(&fixture, &registering(&[TESTS], " "));
    assert!(verdict.contains("must say what it runs"), "unexpected result: {verdict}");
    Ok(())
}
