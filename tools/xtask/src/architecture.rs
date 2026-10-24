// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Enforce unique crate ownership, dependency direction, and authored-source coverage.

use std::{collections::BTreeSet, path::Path};

use crate::{
    error::{Result, require},
    policy::{Architecture, Owner},
    source::{resolve, rust_files},
    workspace::{Workspace, check_inheritance},
};

/// Verify every Cargo member and authored Rust source belongs to exactly one owner.
pub(crate) fn check(root: &Path, policy: &Architecture, workspace: &Workspace) -> Result<()> {
    require(
        policy.schema_version == 1
            && policy.soft_lines > 0
            && policy.soft_lines < policy.hard_lines,
        "architecture: require schema one and increasing positive size limits",
    )?;
    require(
        policy.layers.iter().collect::<BTreeSet<_>>().len() == policy.layers.len()
            && policy.layers.iter().any(|layer| layer == "tooling"),
        "architecture: layers must be unique and include tooling",
    )?;
    let packages: BTreeSet<_> =
        workspace.packages.iter().map(|package| package.name.as_str()).collect();
    let owners: BTreeSet<_> = policy.owners.iter().map(|owner| owner.package.as_str()).collect();
    require(
        packages == owners && policy.owners.len() == owners.len(),
        "architecture: register each workspace package exactly once; remove stale owners",
    )?;
    let mut registered_roots: Vec<std::path::PathBuf> = Vec::new();
    for owner in &policy.owners {
        require(
            !owner.concern.trim().is_empty() && policy.layers.contains(&owner.layer),
            format!("architecture: {} needs a concern and a declared layer", owner.package),
        )?;
        for relative in owner.roots() {
            let path = resolve(root, relative)?;
            require(
                path.is_dir()
                    && registered_roots
                        .iter()
                        .all(|other| !path.starts_with(other) && !other.starts_with(&path)),
                format!("architecture: overlapping or non-directory owner root {relative}"),
            )?;
            registered_roots.push(path);
        }
    }
    let mut registered_files = BTreeSet::new();
    for owner in &policy.owners {
        for relative in &owner.auxiliary_files {
            let path = resolve(root, relative)?;
            require(
                path.is_file()
                    && !path
                        .symlink_metadata()
                        .map_err(|cause| {
                            crate::error::Failure::external("inspect auxiliary source", cause)
                        })?
                        .is_symlink()
                    && !path
                        .extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|value| value.eq_ignore_ascii_case("rs"))
                    && registered_roots.iter().all(|directory| !path.starts_with(directory))
                    && registered_files.insert(path),
                format!(
                    "architecture: auxiliary file {relative} is missing, linked, Rust, or overlaps an owner"
                ),
            )?;
        }
    }
    for package in &workspace.packages {
        let owner =
            policy.owners.iter().find(|owner| owner.package == package.name).ok_or_else(|| {
                crate::error::Failure::Policy(format!("missing owner: {}", package.name))
            })?;
        let expected = resolve(root, &format!("{}/Cargo.toml", owner.root))?;
        require(
            expected.canonicalize().ok() == Path::new(&package.manifest_path).canonicalize().ok(),
            format!("architecture: {} root does not match Cargo manifest", package.name),
        )?;
        check_inheritance(&expected)?;
        for dependency in &package.dependencies {
            if let Some(target) =
                policy.owners.iter().find(|other| other.package == dependency.name)
            {
                check_edge(policy, owner, target, dependency.kind.as_deref())?;
            } else {
                require(
                    dependency.path.is_none(),
                    format!(
                        "{}: external path dependency {} is not reproducible; pin a published source",
                        package.name, dependency.name
                    ),
                )?;
            }
        }
    }
    require(!policy.source_roots.is_empty(), "architecture: source roots must not be empty")?;
    let mut seen = BTreeSet::new();
    for relative in &policy.source_roots {
        for path in rust_files(&resolve(root, relative)?)? {
            require(
                seen.insert(path.clone()),
                format!("architecture: overlapping source roots at {}", path.display()),
            )?;
            let matching = policy.owners.iter().filter(|owner| owner.owns(root, &path)).count();
            require(
                matching == 1,
                format!("{}: authored Rust source needs exactly one crate owner", path.display()),
            )?;
        }
    }
    for owner in &policy.owners {
        for relative in owner.roots() {
            require(
                rust_files(&resolve(root, relative)?)?.iter().all(|path| seen.contains(path)),
                format!("{}: source files fall outside architecture scope", owner.package),
            )?;
        }
    }
    Ok(())
}

/// Reject reversed production edges and every runtime dependency on tooling.
pub(crate) fn check_edge(
    policy: &Architecture,
    owner: &Owner,
    target: &Owner,
    kind: Option<&str>,
) -> Result<()> {
    require(
        owner.layer == "tooling" || target.layer != "tooling",
        format!("{} must not depend on repository tooling {}", owner.package, target.package),
    )?;
    if kind == Some("dev") {
        return Ok(());
    }
    let position = |layer: &String| policy.layers.iter().position(|item| item == layer);
    require(
        position(&owner.layer).zip(position(&target.layer)).is_some_and(|(from, to)| from >= to),
        format!(
            "dependency direction: {} ({}) -> {} ({})",
            owner.package, owner.layer, target.package, target.layer
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Auxiliary sources require unique custody and retain authored Rust scope checks.
    #[test]
    fn auxiliary_roots_cannot_overlap_escape_or_hide_rust() -> Result<()> {
        let mut fixture = crate::fixtures::Repository::new()?;
        crate::fixtures::write(fixture.root(), "installer/worker/main.ts", "export {}")?;
        fixture.architecture.owners[0].auxiliary_roots.push("installer/worker".into());
        check(fixture.root(), &fixture.architecture, &fixture.workspace)?;
        fixture.architecture.owners[0].auxiliary_roots.push("installer".into());
        assert!(check(fixture.root(), &fixture.architecture, &fixture.workspace).is_err());
        assert!(fixture.architecture.owners[0].auxiliary_roots.pop().is_some());
        fixture.architecture.owners[0].auxiliary_roots.push("../outside".into());
        assert!(check(fixture.root(), &fixture.architecture, &fixture.workspace).is_err());
        assert!(fixture.architecture.owners[0].auxiliary_roots.pop().is_some());
        crate::fixtures::write(fixture.root(), "installer/worker/hidden.rs", "fn hidden() {}")?;
        assert!(check(fixture.root(), &fixture.architecture, &fixture.workspace).is_err());
        fixture.architecture.source_roots.push("installer/worker".into());
        check(fixture.root(), &fixture.architecture, &fixture.workspace)
    }

    /// Reject orphan Rust files and missing/duplicate crate owners in real trees.
    #[test]
    fn authored_sources_require_exact_workspace_ownership() -> Result<()> {
        let mut fixture = crate::fixtures::Repository::new()?;
        check(fixture.root(), &fixture.architecture, &fixture.workspace)?;
        fixture.architecture.owners.clear();
        assert!(check(fixture.root(), &fixture.architecture, &fixture.workspace).is_err());
        let fixture = crate::fixtures::Repository::new()?;
        crate::fixtures::write(fixture.root(), "crates/orphan.rs", "fn orphan() {}")?;
        assert!(check(fixture.root(), &fixture.architecture, &fixture.workspace).is_err());
        Ok(())
    }

    /// Require source-root coverage and disallow machine-local external dependencies.
    #[test]
    fn coverage_and_dependency_provenance_cannot_be_bypassed() -> Result<()> {
        let mut fixture = crate::fixtures::Repository::new()?;
        fixture.architecture.source_roots.clear();
        assert!(check(fixture.root(), &fixture.architecture, &fixture.workspace).is_err());
        let mut fixture = crate::fixtures::Repository::new()?;
        fixture.workspace.packages[0].dependencies.push(crate::workspace::Dependency {
            rename: None,
            name: "external".into(),
            kind: None,
            path: Some("machine-local".into()),
        });
        assert!(check(fixture.root(), &fixture.architecture, &fixture.workspace).is_err());
        Ok(())
    }

    /// Prove forbidden dependency direction and tooling leakage are rejected.
    #[test]
    fn layer_edges_preserve_domain_and_runtime_boundaries() -> Result<()> {
        let policy = Architecture {
            schema_version: 1,
            soft_lines: 350,
            hard_lines: 500,
            source_roots: vec![],
            layers: vec!["domain".into(), "host".into(), "tooling".into()],
            owners: vec![],
        };
        let owner = |layer: &str| Owner {
            package: layer.into(),
            root: layer.into(),
            auxiliary_roots: Vec::new(),
            auxiliary_files: Vec::new(),
            layer: layer.into(),
            concern: layer.into(),
        };
        assert!(check_edge(&policy, &owner("domain"), &owner("host"), None).is_err());
        assert!(check_edge(&policy, &owner("host"), &owner("tooling"), Some("dev")).is_err());
        check_edge(&policy, &owner("host"), &owner("domain"), None)?;
        check_edge(&policy, &owner("domain"), &owner("host"), Some("dev"))?;
        Ok(())
    }
}
