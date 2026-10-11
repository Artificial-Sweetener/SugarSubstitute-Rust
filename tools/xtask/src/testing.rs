// SPDX-License-Identifier: GPL-3.0-or-later
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
}

impl<'ast> Visit<'ast> for Inventory {
    /// Count test functions and retain ignored identities for actionable rejection.
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if item.attrs.iter().any(|attribute| {
            attribute.path().segments.last().is_some_and(|segment| segment.ident == "test")
        }) {
            self.count += 1;
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

/// Parse test source rather than mistaking comments or strings for executable tests.
pub(crate) fn test_count(source: &str) -> Result<usize> {
    let parsed = syn::parse_file(source)
        .map_err(|error| Failure::external("parse Rust test inventory", error))?;
    let mut inventory = Inventory::default();
    inventory.visit_file(&parsed);
    require(
        inventory.ignored.is_empty(),
        format!("ignored tests require an implemented exception policy: {:?}", inventory.ignored),
    )?;
    Ok(inventory.count)
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
        for path in rust_files(&resolve(root, &owner.root)?)? {
            count += test_count(&read(&path)?)?;
        }
        require(
            count >= owner.minimum_tests,
            format!(
                "{}: discovered {count} tests; require at least {}",
                owner.package, owner.minimum_tests
            ),
        )?;
        eprintln!("test inventory: {} ({count} functions)", owner.package);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
