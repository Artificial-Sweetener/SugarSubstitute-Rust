// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Enforce one execution adapter before application-owned concurrency can grow.

use std::path::Path;

use serde::Deserialize;

use crate::{
    error::{Result, read, require},
    execution_source,
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
            let findings = execution_source::findings(&read(&path)?)?;
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
