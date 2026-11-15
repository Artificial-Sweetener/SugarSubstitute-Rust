// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Enforce one execution adapter before application-owned concurrency can grow.

use std::{collections::BTreeSet, path::Path};

use serde::Deserialize;

use crate::{
    error::{Result, read, require},
    execution_exports, execution_source,
    policy::Architecture,
    source::{resolve, rust_files},
    testing_scope,
    workspace::Workspace,
};

#[path = "execution_copy_fixture.rs"]
mod copy_fixture;

/// Register the single implemented scheduler owner; absence forbids scheduling everywhere.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    pub(crate) schema_version: u32,
    pub(crate) owner: Option<String>,
    /// The files that run tests side by side on scoped threads.
    #[serde(default)]
    pub(crate) test_scheduling: Vec<TestScheduling>,
}

/// One file that runs tests side by side, and what it runs.
///
/// A registered file may use `std::thread::scope` and no other scheduling primitive: a scoped
/// thread ends before the function that started it returns, so no thread outlives its test.
/// The file's scheduling is in test-only source. Code under test still schedules through the
/// execution owner.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TestScheduling {
    /// The file, relative to the workspace root, with `/` separators.
    pub(crate) path: String,
    /// What the file runs side by side.
    pub(crate) runs: String,
}

/// The only scheduling primitive a registered test-scheduling file may use.
const SCOPED_THREADS: &str = "std::thread::scope";

/// Require a dedicated adapter and scan all authored sources, including tests and other targets.
pub(crate) fn check(
    root: &Path,
    policy: &Policy,
    architecture: &Architecture,
    workspace: &Workspace,
) -> Result<()> {
    require(policy.schema_version == 1, "execution: unsupported policy schema")?;
    if let Some(package) = &policy.owner {
        let registered = architecture.owners.iter().find(|owner| &owner.package == package);
        require(
            registered.is_some_and(|owner| {
                owner.layer == "adapter"
                    && owner.concern == "Application execution and task lifetime"
            }),
            "execution: owner must be one registered dedicated adapter with concern 'Application execution and task lifetime'",
        )?;
    }
    for entry in &policy.test_scheduling {
        require(
            !entry.runs.trim().is_empty(),
            format!(
                "execution: test scheduling of {} must say what it runs side by side",
                entry.path
            ),
        )?;
    }
    let mut scheduled = BTreeSet::new();
    for owner in &architecture.owners {
        let permitted = policy.owner.as_deref() == Some(owner.package.as_str());
        let package = workspace.packages.iter().find(|package| package.name == owner.package);
        require(package.is_some(), format!("execution: missing package {}", owner.package))?;
        if permitted && let Some(package) = package {
            let findings =
                execution_exports::findings(&resolve(root, &owner.root)?, &package.dependencies)?;
            require(
                findings.is_empty(),
                format!(
                    "execution: {} exports raw scheduling capabilities: {}",
                    owner.package,
                    findings.join(", ")
                ),
            )?;
        }
        if !permitted && let Some(package) = package {
            for dependency in &package.dependencies {
                require(
                    !execution_source::restricted_crate(&dependency.name.replace('-', "_")),
                    format!(
                        "execution: {} declares scheduler dependency {}; depend on the execution adapter's typed ports instead",
                        owner.package, dependency.name
                    ),
                )?;
            }
        }
        for path in rust_files(&resolve(root, &owner.root)?)? {
            let source = read(&path)?;
            let qualification = execution_source::qualification_producer_findings(&source)?;
            let actual_child = permitted && path.strip_prefix(root).ok().is_some_and(|path| {
                path == Path::new(
                    "crates/sugarsubstitute-execution/src/windows_lifecycle_pipe/captured/child.rs",
                )
            });
            require(
                qualification.is_empty() || actual_child,
                format!(
                    "execution: {} constructs registered Desktop creator authority outside the authenticated native child producer: {}",
                    path.strip_prefix(root).unwrap_or(&path).display(),
                    qualification.join(", ")
                ),
            )?;
            let fixtures = execution_source::qualification_fixture_findings(&source)?;
            let fixture_owner = permitted
                && path.strip_prefix(root).ok().is_some_and(|path| {
                    path == Path::new(
                        "crates/sugarsubstitute-execution/src/preview_arming/capture_tests.rs",
                    ) || path
                        == Path::new(
                            "crates/sugarsubstitute-execution/src/preview_arming/binding.rs",
                        )
                });
            require(
                fixtures.is_empty() || fixture_owner,
                format!(
                    "execution: {} accesses private preview owner fixture authority: {}",
                    path.strip_prefix(root).unwrap_or(&path).display(),
                    fixtures.join(", ")
                ),
            )?;
            let imports = copy_fixture::import_findings(root, &path, &source)?;
            require(
                imports.is_empty(),
                format!(
                    "execution: {} imports test-only Report source outside its exact gated parent: {}",
                    path.strip_prefix(root).unwrap_or(&path).display(),
                    imports.join(", ")
                ),
            )?;
            let copy_fixtures = execution_source::report_copy_fixture_findings(&source)?;
            require(
                copy_fixtures.is_empty() || copy_fixture::permitted(root, &owner.package, &path)?,
                format!(
                    "execution: {} accesses test-only Report request metadata outside its exact gated owner: {}",
                    path.strip_prefix(root).unwrap_or(&path).display(),
                    copy_fixtures.join(", ")
                ),
            )?;
            if !permitted && owner.package != "sugarsubstitute-contracts" {
                let producer = execution_source::resource_producer_findings(&source)?;
                require(
                    producer.is_empty(),
                    format!(
                        "execution: {} constructs original execution resources outside the contract and execution owners: {}",
                        path.strip_prefix(root).unwrap_or(&path).display(),
                        producer.join(", ")
                    ),
                )?;
            }
            let findings = execution_source::findings(&source)?;
            if !permitted && !findings.is_empty() {
                let relative = relative(root, &path);
                require(
                    policy.test_scheduling.iter().any(|entry| entry.path == relative),
                    format!(
                        "execution: {} uses scheduling primitives outside the execution owner: {}",
                        path.strip_prefix(root).unwrap_or(&path).display(),
                        findings.join(", ")
                    ),
                )?;
                require(
                    findings.iter().all(|finding| finding == SCOPED_THREADS),
                    format!(
                        "execution: {relative} is registered for test scheduling and may use only {SCOPED_THREADS}: {}",
                        findings.join(", ")
                    ),
                )?;
                require(
                    test_only(&resolve(root, &owner.root)?, &path, &findings)?,
                    format!(
                        "execution: {relative} is registered for test scheduling but schedules outside test-only code"
                    ),
                )?;
                scheduled.insert(relative);
            }
        }
    }
    for entry in &policy.test_scheduling {
        require(
            scheduled.contains(&entry.path),
            format!(
                "execution: {} is registered for test scheduling but does not use {SCOPED_THREADS}; remove its registration",
                entry.path
            ),
        )?;
    }
    Ok(())
}

/// `path` relative to the workspace root, with `/` separators, as policies spell files.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Whether every scheduling use in `path` is in source only test builds compile.
fn test_only(crate_root: &Path, path: &Path, findings: &[String]) -> Result<bool> {
    for scoped in testing_scope::scoped_sources(crate_root)? {
        if scoped.path == path {
            return Ok(execution_source::findings(&scoped.source)? == findings);
        }
    }
    Ok(false)
}
