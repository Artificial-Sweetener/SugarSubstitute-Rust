// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enforce one native widget owner and explicit reviewed platform/composition boundaries.

use crate::{
    error::{Result, read, require},
    policy::Architecture,
    source::{fingerprint, resolve},
    ui_source::{self, Capability},
    workspace::Workspace,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Exact source registration prevents an existing platform dependency becoming a UI escape hatch.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    schema_version: u32,
    owner: String,
    dependencies: Vec<Dependency>,
    boundaries: Vec<Boundary>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dependency {
    package: String,
    owners: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Boundary {
    owner: String,
    concern: String,
    sources: Vec<String>,
    capabilities: Vec<Capability>,
    source_sha256: String,
}

/// Reject unknown dependency edges, unregistered UI construction and stale boundary reviews.
pub(crate) fn check(
    root: &Path,
    policy: &Policy,
    architecture: &Architecture,
    workspace: &Workspace,
) -> Result<()> {
    require(policy.schema_version == 1, "UI: unsupported policy schema")?;
    require(
        architecture.owners.iter().any(|o| {
            o.package == policy.owner
                && o.layer == "adapter"
                && o.concern == "Shared native widget rendering and interaction rules"
        }),
        "UI: register one dedicated native widget adapter",
    )?;
    check_dependencies(policy, workspace)?;
    let mut registered = BTreeMap::new();
    for boundary in &policy.boundaries {
        let owner = architecture.owners.iter().find(|o| o.package == boundary.owner);
        require(
            owner.is_some() && boundary.owner != policy.owner,
            "UI: boundary needs a distinct registered owner",
        )?;
        require(
            !boundary.concern.trim().is_empty(),
            "UI: boundary needs an ownership explanation",
        )?;
        let capabilities: BTreeSet<_> = boundary.capabilities.iter().copied().collect();
        require(
            !capabilities.is_empty() && capabilities.len() == boundary.capabilities.len(),
            "UI: duplicate or empty capabilities",
        )?;
        require(
            fingerprint(root, &boundary.sources)? == boundary.source_sha256,
            "UI: boundary source changed; review its UI ownership",
        )?;
        let package = workspace.packages.iter().find(|p| p.name == boundary.owner);
        require(package.is_some(), "UI: boundary has no Cargo package")?;
        let Some(package) = package else {
            continue;
        };
        let mut observed = BTreeSet::new();
        for source in &boundary.sources {
            let path = resolve(root, source)?;
            require(
                owner.is_some_and(|o| o.owns(root, &path))
                    && path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs")),
                format!("UI: boundary source is outside its Rust owner: {source}"),
            )?;
            observed.extend(ui_source::findings(&read(&path)?, &package.dependencies)?);
            require(
                registered.insert(path, (&boundary.owner, capabilities.clone())).is_none(),
                "UI: duplicate source registration",
            )?;
        }
        require(
            observed == capabilities,
            "UI: boundary capabilities changed or contain unused authority",
        )?;
    }
    let roots = ["crates", "tools", "native", "installer"].map(String::from);
    let mut violations = Vec::new();
    for path in crate::authored_sources::sources(root, architecture, &roots, &["rs"])? {
        let owners: Vec<_> =
            architecture.owners.iter().filter(|owner| owner.owns(root, &path)).collect();
        require(
            owners.len() == 1,
            format!("UI: {} requires exactly one authored owner", path.display()),
        )?;
        let owner = owners[0];
        let package = workspace.packages.iter().find(|p| p.name == owner.package);
        require(package.is_some(), "UI: missing workspace owner")?;
        let Some(package) = package else {
            continue;
        };
        let found = ui_source::findings(&read(&path)?, &package.dependencies)?;
        if owner.package == policy.owner {
            require(
                !found.contains(&Capability::Webview) && !found.contains(&Capability::Window),
                "UI: widget owner must not create windows or webviews",
            )?;
            continue;
        }
        if let Some((registered_owner, allowed)) = registered.remove(&path) {
            if *registered_owner != owner.package || found.is_empty() || !found.is_subset(&allowed)
            {
                violations
                    .push(format!("stale or mismatched boundary {}: {found:?}", path.display()));
            }
        } else if !found.is_empty() {
            violations.push(format!("{} constructs {found:?} outside the shared UI or a reviewed platform/composition boundary", path.display()));
        }
    }
    require(
        violations.is_empty(),
        format!("UI: invalid source routes:\n{}", violations.join("\n")),
    )?;
    require(registered.is_empty(), "UI: boundary source was not inspected")
}

/// Every canonical toolkit dependency has an exact, non-stale set of consuming owners.
fn check_dependencies(policy: &Policy, workspace: &Workspace) -> Result<()> {
    let mut expected = BTreeMap::<String, BTreeSet<String>>::new();
    for package in &workspace.packages {
        for dependency in &package.dependencies {
            if ui_source::dependency(&dependency.name).is_some() {
                expected.entry(dependency.name.clone()).or_default().insert(package.name.clone());
            }
        }
    }
    let mut registered = BTreeMap::new();
    for dependency in &policy.dependencies {
        let owners: BTreeSet<_> = dependency.owners.iter().cloned().collect();
        require(
            ui_source::dependency(&dependency.package).is_some()
                && !owners.is_empty()
                && owners.len() == dependency.owners.len(),
            "UI: unknown dependency or duplicate owners",
        )?;
        require(
            registered.insert(dependency.package.clone(), owners).is_none(),
            "UI: duplicate dependency",
        )?;
    }
    require(
        expected == registered,
        "UI: toolkit dependency ownership changed; use the shared UI owner or review the native adapter boundary",
    )
}

#[cfg(test)]
mod tests;
