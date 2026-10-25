// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect network access through shared authored-source discovery and actual Cargo bindings.
use super::{EXTENSIONS, access};
use crate::{
    authored_sources, capability_source,
    error::{Failure, Result, read, require},
    policy::Architecture,
    workspace::{Dependency, Workspace},
};
use std::{collections::BTreeSet, path::Path};

/// Actual source findings and npm manifests, independent of active targets and Git tracking.
pub(super) struct Inventory {
    pub(super) sources: Vec<(String, Vec<String>)>,
    pub(super) manifests: BTreeSet<String>,
}

/// Use the same source ownership and exact dependency exclusion as other infrastructure gates.
pub(super) fn inspect(
    root: &Path,
    architecture: &Architecture,
    workspace: &Workspace,
    roots: &[String],
) -> Result<Inventory> {
    let mut inventory = Inventory { sources: Vec::new(), manifests: BTreeSet::new() };
    let mut extensions = EXTENSIONS.to_vec();
    extensions.push("json");
    for path in authored_sources::sources(root, architecture, roots, &extensions)? {
        let relative = path
            .strip_prefix(root)
            .map_err(|cause| Failure::external("resolve network source", cause))?
            .to_string_lossy()
            .replace('\\', "/");
        let owners: Vec<_> =
            architecture.owners.iter().filter(|owner| owner.owns(root, &path)).collect();
        require(
            owners.len() == 1,
            format!("networking: {relative} needs exactly one authored owner"),
        )?;
        let mut extension =
            path.extension().and_then(|part| part.to_str()).unwrap_or("").to_ascii_lowercase();
        if path
            .file_name()
            .and_then(|part| part.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("package.json"))
        {
            inventory.manifests.insert(relative);
            continue;
        }
        if extension == "json" {
            continue;
        }
        if path
            .file_name()
            .and_then(|part| part.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("CMakeLists.txt"))
        {
            extension = "cmake".into();
        }
        let source = read(&path)?;
        let findings = if extension == "rs" {
            let package = workspace
                .packages
                .iter()
                .find(|package| package.name == owners[0].package)
                .ok_or_else(|| {
                    Failure::Policy("networking: source owner is not a Cargo member".into())
                })?;
            rust(&source, &package.dependencies)?
        } else {
            access::auxiliary(&source, &extension)
        };
        if !findings.is_empty() {
            inventory.sources.push((relative, findings));
        }
    }
    Ok(inventory)
}

/// Renamed declarations retain their real client identity even without an explicit import.
pub(super) fn rust(source: &str, dependencies: &[Dependency]) -> Result<Vec<String>> {
    let bindings: Vec<_> = dependencies
        .iter()
        .filter_map(|dependency| {
            dependency
                .rename
                .as_ref()
                .map(|binding| (binding.replace('-', "_"), dependency.name.replace('-', "_")))
        })
        .collect();
    capability_source::findings_with_literals(
        source,
        access::path,
        access::fragments,
        access::ordinary,
        Some(access::literal),
        &bindings,
    )
}
