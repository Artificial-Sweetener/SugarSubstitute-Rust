// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect real source and public interfaces once for both enforcement and review evidence.
use super::{EXTENSIONS, access, dependencies};
use crate::{
    authored_sources,
    error::{Failure, Result, read, require},
    execution_exports,
    policy::Architecture,
    source::{fingerprint, resolve},
    workspace::Workspace,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Exact constructor/method/export access from one authored source file.
#[derive(Serialize)]
pub(crate) struct Source {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) layer: String,
    pub(crate) source_sha256: String,
    pub(crate) capabilities: BTreeSet<String>,
}

/// Read-only review data uses the same inspection performed by the rejecting gate.
#[derive(Serialize)]
pub(crate) struct Inventory {
    pub(crate) sources: Vec<Source>,
    pub(crate) dependencies: Vec<dependencies::Access>,
}

/// Inspect untracked/ignored sources, inactive targets, aliases, macros and exported implementations.
pub(crate) fn collect(
    root: &Path,
    architecture: &Architecture,
    workspace: &Workspace,
    roots: &[String],
) -> Result<Inventory> {
    let paths = authored_sources::sources(root, architecture, roots, EXTENSIONS)?;
    let mut sources = BTreeMap::new();
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .map_err(|cause| Failure::external("resolve storage source", cause))?
            .to_string_lossy()
            .replace('\\', "/");
        let owners: Vec<_> =
            architecture.owners.iter().filter(|owner| owner.owns(root, &path)).collect();
        require(
            owners.len() == 1,
            format!("storage: {relative} needs exactly one authored owner"),
        )?;
        let owner = owners[0];
        let package =
            workspace.packages.iter().find(|package| package.name == owner.package).ok_or_else(
                || Failure::Policy("storage source owner is not a Cargo member".into()),
            )?;
        let mut extension =
            path.extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("CMakeLists.txt"))
        {
            extension = "cmake".into();
        }
        let source = read(&path)?;
        let capabilities = if extension == "rs" {
            access::rust(&source, &package.dependencies)?
        } else {
            access::script(&source, &extension)
        };
        sources.insert(
            relative.clone(),
            Source {
                path: relative.clone(),
                owner: owner.package.clone(),
                layer: owner.layer.clone(),
                source_sha256: fingerprint(root, &[relative])?,
                capabilities: capabilities.into_iter().collect(),
            },
        );
    }
    for package in &workspace.packages {
        let owner = architecture
            .owners
            .iter()
            .find(|owner| owner.package == package.name)
            .ok_or_else(|| Failure::Policy("storage: unknown public API owner".into()))?;
        for finding in execution_exports::findings_with(
            &resolve(root, &owner.root)?,
            &package.dependencies,
            access::exported,
        )? {
            let (file, detail) = finding.split_once(": ").ok_or_else(|| {
                Failure::Policy("storage: malformed public capability finding".into())
            })?;
            let relative = Path::new(file)
                .strip_prefix(root)
                .map_err(|cause| Failure::external("resolve public storage source", cause))?
                .to_string_lossy()
                .replace('\\', "/");
            let source = sources.get_mut(&relative).ok_or_else(|| {
                Failure::Policy(format!(
                    "storage: public source {relative} lacks authored coverage"
                ))
            })?;
            source.capabilities.insert(format!("export {detail}"));
        }
    }
    let manifest = root.join("installer/package.json");
    if manifest.is_file() {
        let package: serde_json::Value = serde_json::from_str(&read(&manifest)?)
            .map_err(|cause| Failure::external("decode executable npm scripts", cause))?;
        let owners: Vec<_> =
            architecture.owners.iter().filter(|owner| owner.owns(root, &manifest)).collect();
        require(owners.len() == 1, "storage: npm scripts require one exact source owner")?;
        let mut capabilities = BTreeSet::new();
        if let Some(scripts) = package["scripts"].as_object() {
            for (name, body) in scripts {
                let body = body
                    .as_str()
                    .ok_or_else(|| Failure::Policy("storage: npm script must be text".into()))?;
                capabilities.extend(
                    access::script(body, "mjs")
                        .into_iter()
                        .map(|item| format!("npm script {name}: {item}")),
                );
            }
        }
        let relative = "installer/package.json".to_owned();
        sources.insert(
            relative.clone(),
            Source {
                path: relative.clone(),
                owner: owners[0].package.clone(),
                layer: owners[0].layer.clone(),
                source_sha256: fingerprint(root, &[relative])?,
                capabilities,
            },
        );
    }
    Ok(Inventory {
        sources: sources.into_values().filter(|source| !source.capabilities.is_empty()).collect(),
        dependencies: dependencies::inventory(root, architecture, workspace)?,
    })
}
