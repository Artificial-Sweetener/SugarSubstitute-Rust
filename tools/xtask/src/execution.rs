// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Enforce one execution adapter before application-owned concurrency can grow.

use std::path::Path;

use serde::Deserialize;

use crate::{
    error::{Result, read, require},
    execution_exports, execution_source,
    policy::Architecture,
    source::{resolve, rust_files},
    workspace::Workspace,
};

/// Register the single implemented scheduler owner; absence forbids scheduling everywhere.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    pub(crate) schema_version: u32,
    pub(crate) owner: Option<String>,
}

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
            if !permitted {
                require(
                    findings.is_empty(),
                    format!(
                        "execution: {} uses scheduling primitives outside the execution owner: {}",
                        path.strip_prefix(root).unwrap_or(&path).display(),
                        findings.join(", ")
                    ),
                )?;
            }
        }
    }
    Ok(())
}
