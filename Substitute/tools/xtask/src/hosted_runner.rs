// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Keep tests that qualify hardware a hosted CI runner lacks out of that runner's test legs.
//!
//! A hosted runner has no hardware GPU and cannot display a window, so a test that qualifies
//! either can only fail there. The testing policy names each such test with what it needs.
//! `verify` runs every one of them wherever the hardware exists. On a runner that declares
//! itself hosted through [`VARIABLE`], it skips exactly those tests by their full path and
//! says so by name, so a green hosted run never reads as proof of them.

use crate::policy::{HostedRunnerTest, Testing};

/// The environment variable a hosted runner's workflow sets to `1`.
pub(crate) const VARIABLE: &str = "SUGARSUBSTITUTE_HOSTED_RUNNER";

/// Whether this process runs on a runner that declared itself hosted.
pub(crate) fn declared() -> bool {
    std::env::var_os(VARIABLE).is_some_and(|value| value == "1")
}

/// The arguments a test leg gains on a hosted runner: every listed test skipped by its path.
/// Nothing is skipped anywhere else.
pub(crate) fn skips(policy: &Testing, hosted: bool) -> Vec<String> {
    if !hosted || policy.hosted_runner.is_empty() {
        return Vec::new();
    }
    std::iter::once("--".to_owned())
        .chain(
            policy.hosted_runner.iter().flat_map(|test| ["--skip".to_owned(), test.test.clone()]),
        )
        .collect()
}

/// One line per test a hosted runner does not run, with what the test needs.
pub(crate) fn notices(tests: &[HostedRunnerTest]) -> Vec<String> {
    tests
        .iter()
        .map(|test| {
            format!(
                "not run on this hosted runner: {} {} (needs {})",
                test.package, test.test, test.requires
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> Testing {
        Testing {
            schema_version: 1,
            platforms: Vec::new(),
            owners: Vec::new(),
            real_time: Vec::new(),
            hosted_runner: vec![
                HostedRunnerTest {
                    package: "host".into(),
                    test: "canvas::tests::shares_frames".into(),
                    requires: "a hardware Vulkan adapter".into(),
                },
                HostedRunnerTest {
                    package: "host".into(),
                    test: "setup::tests::shows_a_window".into(),
                    requires: "a desktop that displays windows".into(),
                },
            ],
        }
    }

    /// Every test runs wherever the runner is not declared hosted; a hosted runner skips
    /// exactly the listed tests and names each with what it needs.
    #[test]
    fn only_a_hosted_runner_skips_the_listed_tests_and_it_names_them() {
        let policy = policy();
        assert!(skips(&policy, false).is_empty());
        assert_eq!(
            skips(&policy, true),
            [
                "--",
                "--skip",
                "canvas::tests::shares_frames",
                "--skip",
                "setup::tests::shows_a_window"
            ]
        );
        assert_eq!(
            notices(&policy.hosted_runner),
            [
                "not run on this hosted runner: host canvas::tests::shares_frames (needs a \
                 hardware Vulkan adapter)",
                "not run on this hosted runner: host setup::tests::shows_a_window (needs a \
                 desktop that displays windows)",
            ]
        );
        let none = Testing { hosted_runner: Vec::new(), ..policy };
        assert!(skips(&none, true).is_empty(), "an empty list adds no separator");
    }
}
