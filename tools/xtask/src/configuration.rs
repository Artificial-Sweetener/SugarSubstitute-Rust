// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Govern known configuration access routes across every authored target and language.
//!
//! Rust paths use parsed imports, aliases and macro tokens. Other languages use a
//! conservative token inventory, including quoted properties. This is an access gate,
//! not proof of arbitrary computed script behavior or semantic setting equivalence.
mod access;
mod exports;
#[cfg(test)]
mod tests;
use crate::{
    error::{Failure, Result, read, require},
    policy::Architecture,
    source::{fingerprint, resolve, source_files},
    workspace::Workspace,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Require the implemented document owner and finite reviewed boundary access.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    pub(crate) schema_version: u32,
    pub(crate) owner: String,
    pub(crate) records: Vec<Record>,
    pub(crate) surfaces: Vec<Surface>,
}
/// Register native configuration surfaces without changing other gates' source scope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Surface {
    pub(crate) root: String,
    pub(crate) owner: String,
}
/// Review one exact source and its supported low-level capabilities.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) source_sha256: String,
    pub(crate) capabilities: BTreeSet<access::Capability>,
    pub(crate) rationale: String,
}

/// Reject competing configuration backends by dependency identity, including aliases
/// and optional, target-specific, build and development declarations.
pub(crate) fn check(
    root: &Path,
    architecture: &Architecture,
    workspace: &Workspace,
    policy: &Policy,
) -> Result<()> {
    require(policy.schema_version == 1, "configuration: unsupported schema")?;
    require(
        architecture
            .owners
            .iter()
            .any(|owner| owner.package == policy.owner && owner.layer == "adapter"),
        "configuration: missing adapter owner",
    )?;
    let mut roots = architecture
        .owners
        .iter()
        .flat_map(|owner| owner.roots().map(|root| (root.to_owned(), owner.package.as_str())))
        .collect::<Vec<_>>();
    for surface in &policy.surfaces {
        let path = resolve(root, &surface.root)?;
        require(
            path.is_dir() && architecture.owners.iter().any(|owner| owner.package == surface.owner),
            "configuration: unknown native surface owner or non-directory root",
        )?;
        require(
            roots.iter().all(|(other, _)| {
                !path.starts_with(root.join(other)) && !root.join(other).starts_with(&path)
            }),
            "configuration: overlapping source surfaces",
        )?;
        roots.push((surface.root.clone(), surface.owner.as_str()));
    }
    let native = root.join("native");
    if native.is_dir() {
        for path in source_files(&native, access::EXTENSIONS)? {
            require(
                roots.iter().filter(|(relative, _)| path.starts_with(root.join(relative))).count()
                    == 1,
                format!("configuration: missing native source coverage {}", path.display()),
            )?;
        }
    }
    for package in &workspace.packages {
        for dependency in &package.dependencies {
            require(
                !access::backend(&dependency.name.replace('-', "_")),
                format!(
                    "configuration: {} declares competing backend {}",
                    package.name, dependency.name
                ),
            )?;
            if dependency.name == "toml" {
                let tooling = architecture
                    .owners
                    .iter()
                    .any(|owner| owner.package == package.name && owner.layer == "tooling");
                require(
                    dependency.rename.is_none()
                        && (tooling
                            || package.name == policy.owner
                            || package.name == "sugarsubstitute-supervision"),
                    format!(
                        "configuration: {} declares an unowned or renamed startup codec",
                        package.name
                    ),
                )?;
            }
        }
    }
    let mut records = BTreeMap::new();
    for record in &policy.records {
        require(
            records.insert(record.path.as_str(), record).is_none(),
            "configuration: duplicate source",
        )?;
        require(
            !record.rationale.trim().is_empty() && !record.capabilities.is_empty(),
            "configuration: empty review",
        )?;
        let path = resolve(root, &record.path)?;
        let owners: Vec<_> =
            roots.iter().filter(|(relative, _)| path.starts_with(root.join(relative))).collect();
        require(
            owners.len() == 1 && owners[0].1 == record.owner,
            format!("configuration: {} has foreign source ownership", record.path),
        )?;
        require(
            fingerprint(root, std::slice::from_ref(&record.path))? == record.source_sha256,
            format!("configuration: stale source {}", record.path),
        )?;
    }
    let mut used = BTreeMap::new();
    for owner in &architecture.owners {
        for relative in owner.roots().chain(
            policy
                .surfaces
                .iter()
                .filter(|surface| surface.owner == owner.package)
                .map(|surface| surface.root.as_str()),
        ) {
            for path in source_files(&resolve(root, relative)?, access::EXTENSIONS)? {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|cause| Failure::external("resolve configuration source", cause))?
                    .to_string_lossy()
                    .replace('\\', "/");
                let source = read(&path)?;
                let rust = path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"));
                let findings = access::findings(&source, rust)?;
                require(
                    !findings.contains(&access::Capability::Backend),
                    format!(
                        "configuration: {relative} constructs a competing persistent settings backend"
                    ),
                )?;
                require(
                    !findings.contains(&access::Capability::Codec)
                        || owner.package == policy.owner
                        || owner.layer == "tooling"
                        || relative
                            == "crates/sugarsubstitute-supervision/src/installed_frontend/manifest.rs",
                    format!("configuration: {relative} bypasses the authoritative document codec"),
                )?;
                if !findings.is_empty() {
                    let record = records.get(relative.as_str()).ok_or_else(|| {
                        Failure::Policy(format!(
                            "configuration: {relative} has unowned access {findings:?}"
                        ))
                    })?;
                    require(
                        record.capabilities == findings,
                        format!(
                            "configuration: {relative} has stale or missing capability coverage {findings:?}"
                        ),
                    )?;
                    used.insert(relative, findings);
                }
                if rust && owner.package == policy.owner {
                    require(
                        exports::check(&source)?,
                        "configuration: document owner exposes a raw settings/file type",
                    )?;
                }
            }
        }
    }
    require(
        used.keys().map(String::as_str).collect::<BTreeSet<_>>()
            == records.keys().copied().collect(),
        "configuration: remove stale source records",
    )
}
