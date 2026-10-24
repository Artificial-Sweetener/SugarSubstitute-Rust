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
    authored_sources,
    error::{Failure, Result, read, require},
    policy::Architecture,
    source::{fingerprint, resolve},
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
    let roots = ["crates", "tools", "native", "installer"].map(String::from);
    let mut extensions = access::EXTENSIONS.to_vec();
    extensions.push("json");
    let mut paths = Vec::new();
    for path in authored_sources::sources(root, architecture, &roots, &extensions)? {
        require(
            architecture.owners.iter().filter(|owner| owner.owns(root, &path)).count() == 1,
            format!("configuration: {} needs exactly one authored owner", path.display()),
        )?;
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("package.json"))
        {
            for identity in crate::script_dependencies::identities(&read(&path)?)? {
                require(
                    !access::backend(&identity.replace('-', "_")),
                    format!("configuration: independent script settings backend {identity}"),
                )?;
            }
        } else if !path
            .extension()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("json"))
        {
            paths.push(path);
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
            architecture.owners.iter().filter(|owner| owner.owns(root, &path)).collect();
        require(
            owners.len() == 1 && owners[0].package == record.owner,
            format!("configuration: {} has foreign source ownership", record.path),
        )?;
        require(
            fingerprint(root, std::slice::from_ref(&record.path))? == record.source_sha256,
            format!("configuration: stale source {}", record.path),
        )?;
    }
    let mut used = BTreeMap::new();
    let mut missing = Vec::new();
    for owner in &architecture.owners {
        if owner.package == policy.owner {
            let package = workspace
                .packages
                .iter()
                .find(|package| package.name == owner.package)
                .ok_or_else(|| {
                Failure::Policy("configuration: document owner is not a Cargo member".into())
            })?;
            require(
                exports::check(&resolve(root, &owner.root)?, &package.dependencies)?,
                "configuration: document owner exposes a raw settings/file type",
            )?;
        }
        for path in paths.iter().filter(|path| owner.owns(root, path)) {
            let relative = path
                .strip_prefix(root)
                .map_err(|cause| Failure::external("resolve configuration source", cause))?
                .to_string_lossy()
                .replace('\\', "/");
            let source = read(path)?;
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
                    || matches!(
                        relative.as_str(),
                        "crates/sugarsubstitute-supervision/src/installed_frontend/manifest.rs"
                            | "installer/worker/dependency-inspection.mjs"
                    ),
                format!("configuration: {relative} bypasses the authoritative document codec"),
            )?;
            if !findings.is_empty() {
                let Some(record) = records.get(relative.as_str()) else {
                    missing.push(format!("{relative} has unowned access {findings:?}"));
                    continue;
                };
                require(
                    record.capabilities == findings,
                    format!(
                        "configuration: {relative} has stale or missing capability coverage {findings:?}"
                    ),
                )?;
                used.insert(relative, findings);
            }
        }
    }
    require(missing.is_empty(), format!("configuration: unowned access:\n{}", missing.join("\n")))?;
    let unused: Vec<_> = records.keys().copied().filter(|path| !used.contains_key(*path)).collect();
    require(
        unused.is_empty(),
        format!("configuration: remove stale source records:\n{}", unused.join("\n")),
    )
}
