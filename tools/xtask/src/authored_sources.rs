// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Discover authored sources independently of Git, feature selection and directory ownership.
use crate::{
    error::{Failure, Result, read, require},
    policy::Architecture,
    source::{resolve, source_files_excluding},
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
const ROOTS: &[&str] = &["crates", "tools", "native", "installer"];
/// Discover source regardless of Git tracking, active features, architecture registration or case.
pub(crate) fn sources(
    root: &Path,
    architecture: &Architecture,
    source_roots: &[String],
    extensions: &[&str],
) -> Result<BTreeSet<PathBuf>> {
    require(
        source_roots.len() == ROOTS.len()
            && source_roots.iter().map(String::as_str).collect::<BTreeSet<_>>()
                == ROOTS.iter().copied().collect(),
        "authored sources: missing, duplicate or unsupported authored source coverage",
    )?;
    for relative in architecture
        .source_roots
        .iter()
        .map(String::as_str)
        .chain(architecture.owners.iter().flat_map(|owner| owner.roots()))
    {
        require(
            ROOTS.iter().any(|source| Path::new(relative).starts_with(source)),
            format!("authored sources: architecture root {relative} lacks source coverage"),
        )?;
    }
    for owner in &architecture.owners {
        for relative in &owner.auxiliary_files {
            require(
                ROOTS.iter().any(|source| Path::new(relative).starts_with(source)),
                format!("authored sources: auxiliary file {relative} lacks coverage"),
            )?;
        }
    }
    let mut paths = BTreeSet::new();
    let dependencies = root.join("installer/node_modules");
    if dependencies.symlink_metadata().is_ok() {
        dependency_manifest(root)?;
    }
    for relative in source_roots {
        paths.extend(source_files_excluding(
            &resolve(root, relative)?,
            extensions,
            std::slice::from_ref(&dependencies),
        )?);
    }
    Ok(paths)
}

/// The sole installed dependency exclusion requires matching current npm declarations.
fn dependency_manifest(root: &Path) -> Result<()> {
    let decode = |relative| -> Result<serde_json::Value> {
        serde_json::from_str(&read(&resolve(root, relative)?)?)
            .map_err(|cause| Failure::external("decode authored dependency ownership", cause))
    };
    let manifest = decode("installer/package.json")?;
    let lock = decode("installer/package-lock.json")?;
    let installed = &lock["packages"][""];
    require(
        manifest["name"].as_str().is_some_and(|name| !name.is_empty())
            && manifest["version"].as_str().is_some_and(|version| !version.is_empty())
            && manifest["private"] == true
            && lock["lockfileVersion"] == 3
            && lock["name"] == manifest["name"]
            && installed["name"] == manifest["name"]
            && installed["version"] == manifest["version"]
            && ["dependencies", "devDependencies", "optionalDependencies"]
                .iter()
                .all(|kind| installed[*kind] == manifest[*kind]),
        "authored sources: installed dependency root needs matching private npm manifest and schema-three lockfile",
    )
}
