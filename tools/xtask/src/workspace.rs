// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Inspect Cargo's resolved workspace inventory for crate governance.

use std::{collections::BTreeSet, path::Path, process::Command};

use serde::Deserialize;

use crate::error::{Failure, Result, read, require};

/// Retain only workspace members from Cargo's metadata envelope.
#[derive(Deserialize)]
pub(crate) struct Workspace {
    pub(crate) packages: Vec<Package>,
    workspace_members: BTreeSet<String>,
}

/// Preserve package identity, manifest location, and declared dependencies.
#[derive(Deserialize)]
pub(crate) struct Package {
    pub(crate) name: String,
    id: String,
    pub(crate) manifest_path: String,
    pub(crate) dependencies: Vec<Dependency>,
}

/// Identify production edges and reject machine-specific external path bindings.
#[derive(Deserialize)]
pub(crate) struct Dependency {
    pub(crate) name: String,
    pub(crate) rename: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) path: Option<String>,
}

impl Workspace {
    /// Ask Cargo for locked metadata without resolving or building dependencies.
    pub(crate) fn load(root: &Path) -> Result<Self> {
        let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .current_dir(root)
            .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
            .output()
            .map_err(|error| Failure::external("inspect Cargo workspace", error))?;
        require(
            output.status.success(),
            format!("cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr)),
        )?;
        let mut workspace: Self = serde_json::from_slice(&output.stdout)
            .map_err(|error| Failure::external("decode Cargo metadata", error))?;
        workspace.packages.retain(|package| workspace.workspace_members.contains(&package.id));
        Ok(workspace)
    }
}

/// Require member metadata/lints to inherit the repository's authoritative values.
pub(crate) fn check_inheritance(manifest: &Path) -> Result<()> {
    let value: toml::Value = toml::from_str(&read(manifest)?)
        .map_err(|error| Failure::external("parse member manifest", error))?;
    for key in ["version", "edition", "rust-version", "license", "authors", "publish"] {
        require(
            value
                .get("package")
                .and_then(|package| package.get(key))
                .and_then(|field| field.get("workspace"))
                .and_then(toml::Value::as_bool)
                == Some(true),
            format!("{}: package.{key} must inherit workspace metadata", manifest.display()),
        )?;
    }
    require(
        value.get("lints").and_then(|lints| lints.get("workspace")).and_then(toml::Value::as_bool)
            == Some(true),
        format!("{}: inherit workspace lints", manifest.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reject member metadata or lints that drift away from the workspace owner.
    #[test]
    fn member_manifests_must_inherit_metadata_and_lints() -> Result<()> {
        let fixture = crate::fixtures::Repository::new()?;
        let manifest = fixture.root().join("crates/host/Cargo.toml");
        check_inheritance(&manifest)?;
        let original = read(&manifest)?;
        for source in [
            original.replace("version.workspace = true", "version = '0.2.0'"),
            original.replace("[lints]\nworkspace = true", "[lints]\nworkspace = false"),
        ] {
            crate::fixtures::write(fixture.root(), "crates/host/Cargo.toml", &source)?;
            assert!(check_inheritance(&manifest).is_err());
        }
        Ok(())
    }
}
