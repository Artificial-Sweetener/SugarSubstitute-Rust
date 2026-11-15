// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Require owner-local nonempty Rust test inventories and reject ignored tests.

use std::{collections::BTreeSet, path::Path};

use syn::visit::Visit;

use crate::{
    error::{Failure, Result, read, require},
    policy::{Architecture, Testing},
    source::{resolve, rust_files},
};

/// Discover ordinary and qualified Rust test attributes throughout nested modules.
#[derive(Default)]
struct Inventory {
    count: usize,
    ignored: Vec<String>,
    names: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for Inventory {
    /// Count test functions and retain ignored identities for actionable rejection.
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if item.attrs.iter().any(|attribute| {
            attribute.path().segments.last().is_some_and(|segment| segment.ident == "test")
        }) {
            self.count += 1;
            self.names.insert(item.sig.ident.to_string());
            if item.attrs.iter().any(|attribute| ignores_test(&attribute.meta)) {
                self.ignored.push(item.sig.ident.to_string());
            }
        }
        syn::visit::visit_item_fn(self, item);
    }
}

/// Detect conditional ignore attributes instead of allowing platform-specific bypasses.
fn ignores_test(meta: &syn::Meta) -> bool {
    if meta.path().is_ident("ignore") {
        return true;
    }
    if let syn::Meta::List(list) = meta
        && list.path.is_ident("cfg_attr")
    {
        return list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .map_or(true, |arguments| arguments.iter().skip(1).any(ignores_test));
    }
    false
}

/// Discover one source file's tests, rejecting any that is ignored. The source is parsed, so
/// comments and strings are never mistaken for executable tests.
fn inventory(source: &str) -> Result<Inventory> {
    let parsed = syn::parse_file(source)
        .map_err(|error| Failure::external("parse Rust test inventory", error))?;
    let mut inventory = Inventory::default();
    inventory.visit_file(&parsed);
    require(
        inventory.ignored.is_empty(),
        format!("ignored tests require an implemented exception policy: {:?}", inventory.ignored),
    )?;
    Ok(inventory)
}

/// Require each test a hosted runner skips to exist in its package under that name, once,
/// with the hardware it needs said, so the list cannot outlive or misname a test.
fn check_hosted_runner(policy: &Testing, names: &[(String, BTreeSet<String>)]) -> Result<()> {
    let mut listed = BTreeSet::new();
    for test in &policy.hosted_runner {
        let function = test.test.rsplit("::").next().unwrap_or_default();
        require(
            names
                .iter()
                .any(|(package, found)| package == &test.package && found.contains(function)),
            format!("{}: hosted-runner test {} does not exist", test.package, test.test),
        )?;
        require(
            test.test.contains("::") && !test.requires.trim().is_empty(),
            format!(
                "{}: hosted-runner test {} needs its full path and what it needs",
                test.package, test.test
            ),
        )?;
        require(
            listed.insert((test.package.as_str(), test.test.as_str())),
            format!("{}: hosted-runner test {} is listed twice", test.package, test.test),
        )?;
    }
    Ok(())
}

/// Bind every workspace package to owner-local tests and the three product platforms.
pub(crate) fn check(root: &Path, policy: &Testing, architecture: &Architecture) -> Result<()> {
    let platforms: BTreeSet<_> = policy.platforms.iter().map(String::as_str).collect();
    require(
        policy.schema_version == 1
            && platforms == BTreeSet::from(["windows", "macos", "linux"])
            && platforms.len() == policy.platforms.len(),
        "testing: declare Windows, macOS and Linux exactly once",
    )?;
    let expected: BTreeSet<_> =
        architecture.owners.iter().map(|owner| owner.package.as_str()).collect();
    let actual: BTreeSet<_> = policy.owners.iter().map(|owner| owner.package.as_str()).collect();
    require(
        expected == actual && actual.len() == policy.owners.len(),
        "testing: each crate requires exactly one current test owner",
    )?;
    let mut names = Vec::new();
    for owner in &policy.owners {
        require(
            owner.minimum_tests > 0 && !owner.concern.trim().is_empty(),
            format!("{}: tests need a concern and nonzero inventory", owner.package),
        )?;
        require(
            architecture
                .owners
                .iter()
                .any(|entry| entry.package == owner.package && entry.root == owner.root),
            format!("{}: test root must match the authoritative crate", owner.package),
        )?;
        let mut count = 0;
        let mut found = BTreeSet::new();
        for path in rust_files(&resolve(root, &owner.root)?)? {
            let inventory = inventory(&read(&path)?)?;
            count += inventory.count;
            found.extend(inventory.names);
        }
        names.push((owner.package.clone(), found));
        require(
            count >= owner.minimum_tests,
            format!(
                "{}: discovered {count} tests; require at least {}",
                owner.package, owner.minimum_tests
            ),
        )?;
        eprintln!("test inventory: {} ({count} functions)", owner.package);
    }
    check_hosted_runner(policy, &names)?;
    crate::testing_time::check(root, &policy.real_time, architecture)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_count(source: &str) -> Result<usize> {
        Ok(inventory(source)?.count)
    }

    /// Reject missing test owners, empty proof requirements and absent platform scope.
    #[test]
    fn test_policy_requires_owned_nonempty_proof_on_all_platforms() -> Result<()> {
        let fixture = crate::fixtures::Repository::new()?;
        let mut policy = Testing {
            schema_version: 1,
            platforms: vec!["windows".into(), "macos".into(), "linux".into()],
            owners: vec![crate::policy::TestOwner {
                package: "host".into(),
                root: "crates/host".into(),
                concern: "fixture".into(),
                minimum_tests: 1,
            }],
            real_time: Vec::new(),
            hosted_runner: Vec::new(),
        };
        check(fixture.root(), &policy, &fixture.architecture)?;
        policy.owners[0].minimum_tests = 0;
        assert!(check(fixture.root(), &policy, &fixture.architecture).is_err());
        policy.owners[0].minimum_tests = 2;
        assert!(check(fixture.root(), &policy, &fixture.architecture).is_err());
        policy.owners[0].minimum_tests = 1;
        policy.platforms.pop();
        assert!(check(fixture.root(), &policy, &fixture.architecture).is_err());
        Ok(())
    }

    /// A test a hosted runner skips must exist in its package under its full path, once, and
    /// say what it needs, so the list can neither outlive a test nor hide one.
    #[test]
    fn hosted_runner_tests_must_exist_and_say_what_they_need() -> Result<()> {
        use crate::policy::HostedRunnerTest;
        let fixture = crate::fixtures::Repository::new()?;
        let listed = |test: &str, requires: &str| HostedRunnerTest {
            package: "host".into(),
            test: test.into(),
            requires: requires.into(),
        };
        let policy = |hosted_runner| Testing {
            schema_version: 1,
            platforms: vec!["windows".into(), "macos".into(), "linux".into()],
            owners: vec![crate::policy::TestOwner {
                package: "host".into(),
                root: "crates/host".into(),
                concern: "fixture".into(),
                minimum_tests: 1,
            }],
            real_time: Vec::new(),
            hosted_runner,
        };
        let verdict = |tests| check(fixture.root(), &policy(tests), &fixture.architecture);
        verdict(vec![listed("tests::behavior", "a hardware Vulkan adapter")])?;
        for refused in [
            vec![listed("tests::removed", "a hardware Vulkan adapter")],
            vec![listed("behavior", "a hardware Vulkan adapter")],
            vec![listed("tests::behavior", " ")],
            vec![listed("tests::behavior", "a display"), listed("tests::behavior", "a display")],
        ] {
            assert!(verdict(refused).is_err());
        }
        Ok(())
    }

    /// Reject ignored tests and exclude misleading strings/comments from discovery.
    #[test]
    fn discovery_counts_nested_tests_and_rejects_ignored_functions() -> Result<()> {
        assert_eq!(
            test_count(
                "// #[test] fn fake() {}\nconst X: &str = \"#[test]\";\nmod nested { #[test] fn real() {} }"
            )?,
            1
        );
        assert!(test_count("#[test] #[ignore = \"flaky\"] fn skipped() {}").is_err());
        assert!(test_count("#[test] #[cfg_attr(windows, ignore)] fn skipped() {}").is_err());
        assert!(
            test_count("#[test] #[cfg_attr(windows, cfg_attr(unix, ignore))] fn skipped() {}")
                .is_err()
        );
        assert!(test_count("invalid rust!").is_err());
        Ok(())
    }
}
