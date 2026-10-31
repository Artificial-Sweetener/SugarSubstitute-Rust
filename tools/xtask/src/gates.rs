// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own the Cargo legs that `check` and `verify` run after governance and formatting.

/// Strict lints across every feature, the shipped application, and its minimal host.
///
/// The shipped leg passes no feature flags, so it compiles exactly the default feature set an
/// ordinary installer build ships, which feature unification can otherwise hide behind the
/// all-features leg.
pub(crate) const CLIPPY: [&[&str]; 3] = [
    &[
        "clippy",
        "--locked",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ],
    &["clippy", "--locked", "-p", "sugarsubstitute", "--all-targets", "--", "-D", "warnings"],
    &[
        "clippy",
        "--locked",
        "-p",
        "sugarsubstitute",
        "--no-default-features",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ],
];

/// Behavior tests across every feature, the shipped application, and its minimal host library.
pub(crate) const TESTS: [&[&str]; 3] = [
    &["test", "--locked", "--workspace", "--all-features"],
    &["test", "--locked", "-p", "sugarsubstitute"],
    &["test", "--locked", "-p", "sugarsubstitute", "--no-default-features", "--lib"],
];

/// Warning-free Rustdoc for the complete feature surface.
pub(crate) const DOCUMENTATION: &[&str] =
    &["doc", "--locked", "--workspace", "--all-features", "--no-deps"];

#[cfg(test)]
mod tests {
    use super::*;

    /// A leg selects the shipped feature set only when it names the application and no features.
    fn ships_default_features(leg: &[&str]) -> bool {
        leg.windows(2).any(|pair| pair == ["-p", "sugarsubstitute"])
            && !leg
                .iter()
                .any(|argument| argument.starts_with("--") && argument.contains("feature"))
    }

    #[test]
    fn check_and_verify_lint_and_test_the_shipped_default_feature_set() {
        assert_eq!(CLIPPY.iter().filter(|leg| ships_default_features(leg)).count(), 1);
        assert_eq!(TESTS.iter().filter(|leg| ships_default_features(leg)).count(), 1);
        for leg in CLIPPY {
            assert!(leg.contains(&"--locked") && leg.ends_with(&["-D", "warnings"]));
        }
        for leg in TESTS.iter().chain([&DOCUMENTATION]) {
            assert!(leg.contains(&"--locked"));
        }
    }

    #[test]
    fn feature_selection_is_recognized_in_every_spelling() {
        assert!(ships_default_features(&["test", "--locked", "-p", "sugarsubstitute"]));
        for flag in ["--all-features", "--no-default-features", "--features"] {
            assert!(!ships_default_features(&["test", "-p", "sugarsubstitute", flag]));
        }
        assert!(!ships_default_features(&["test", "--locked", "--workspace"]));
    }
}
