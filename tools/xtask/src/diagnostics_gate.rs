// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enforce supported diagnostic routes across authored source and Cargo dependency identities.
//!
//! Parsed Rust paths cover aliases, wrappers, macro tokens and inactive configurations.
//! Auxiliary languages use conservative identifier/literal inspection. These checks
//! identify known sink construction and stdout routes; they do not prove arbitrary
//! computed code or infer a generic file write's semantic purpose. Behavioral and
//! native qualification prove routing, redaction, bounded retention and retirement.
mod source;
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

const EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "c", "cc", "cpp", "cxx", "h",
    "hpp", "hxx", "m", "mm", "html",
];
/// Bind supported routing and persistence owners to an exact source inventory.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    pub(crate) schema_version: u32,
    pub(crate) owner: String,
    pub(crate) routing_entry: String,
    pub(crate) storage_owner: String,
    pub(crate) storage_entry: String,
    pub(crate) worker_entry: String,
    pub(crate) required_roots: Vec<String>,
    pub(crate) script_manifests: Vec<String>,
    pub(crate) records: Vec<Record>,
}
/// Source review is scoped to one actual boundary, never a blanket crate or test exemption.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) source_sha256: String,
    pub(crate) disposition: Disposition,
    pub(crate) rationale: String,
}
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Disposition {
    Routing,
    Storage,
    Capture,
    Fixture,
    Developer,
    Upstream,
}

/// Reject unknown policy shapes, dependency aliases, unauthorized routes and stale boundary records.
pub(crate) fn check(
    root: &Path,
    architecture: &Architecture,
    workspace: &Workspace,
    policy: &Policy,
) -> Result<()> {
    require(policy.schema_version == 1, "diagnostics: unsupported schema")?;
    if root.join("installer/package.json").is_file() {
        require(
            policy.script_manifests.iter().any(|path| path == "installer/package.json"),
            "diagnostics: provisioning dependency manifest is missing from coverage",
        )?;
    }
    for authored in ["native", "installer/worker", "installer/surface"] {
        let directory = root.join(authored);
        if directory.is_dir() {
            for path in source_files(&directory, EXTENSIONS)? {
                require(
                    architecture
                        .owners
                        .iter()
                        .filter(|owner| {
                            owner.roots().any(|owned| path.starts_with(root.join(owned)))
                        })
                        .count()
                        == 1,
                    "diagnostics: authored native or worker source has no unique owner",
                )?;
            }
        }
    }
    require(
        !policy.script_manifests.is_empty()
            && policy.script_manifests.iter().collect::<BTreeSet<_>>().len()
                == policy.script_manifests.len(),
        "diagnostics: script dependency coverage must be nonempty and unique",
    )?;
    for manifest in &policy.script_manifests {
        source::script_dependencies(&read(&resolve(root, manifest)?)?)?;
    }
    require(
        !policy.required_roots.is_empty()
            && policy.required_roots.iter().collect::<BTreeSet<_>>().len()
                == policy.required_roots.len(),
        "diagnostics: required source coverage must be nonempty and unique",
    )?;
    for required in &policy.required_roots {
        let path = resolve(root, required)?;
        require(
            path.is_dir()
                && architecture
                    .owners
                    .iter()
                    .filter(|owner| owner.roots().any(|owned| path.starts_with(root.join(owned))))
                    .count()
                    == 1,
            "diagnostics: required auxiliary source coverage is missing",
        )?;
    }
    let route_owner = architecture
        .owners
        .iter()
        .find(|owner| owner.package == policy.owner && owner.layer == "host")
        .ok_or_else(|| Failure::Policy("diagnostics: routing requires its host owner".into()))?;
    let storage_owner = architecture
        .owners
        .iter()
        .find(|owner| owner.package == policy.storage_owner && owner.layer == "adapter")
        .ok_or_else(|| {
            Failure::Policy("diagnostics: persistence requires its adapter owner".into())
        })?;
    for (entry, owner) in [
        (&policy.routing_entry, route_owner),
        (&policy.worker_entry, route_owner),
        (&policy.storage_entry, storage_owner),
    ] {
        let path = resolve(root, entry)?;
        require(
            path.is_file() && owner.roots().any(|owned| path.starts_with(root.join(owned))),
            "diagnostics: missing or foreign supported entry",
        )?;
    }
    for package in &workspace.packages {
        for dependency in &package.dependencies {
            let identity = dependency.name.replace('-', "_");
            require(
                !source::logging_crate(&identity)
                    || identity == "tracing_subscriber" && package.name == policy.owner,
                format!(
                    "diagnostics: {} declares independent logger {}",
                    package.name, dependency.name
                ),
            )?;
        }
    }
    let mut records = BTreeMap::new();
    for record in &policy.records {
        require(
            records.insert(record.path.as_str(), record).is_none(),
            "diagnostics: duplicate boundary",
        )?;
        let path = resolve(root, &record.path)?;
        let owners: Vec<_> = architecture
            .owners
            .iter()
            .filter(|owner| owner.roots().any(|owned| path.starts_with(root.join(owned))))
            .collect();
        require(
            owners.len() == 1 && owners[0].package == record.owner,
            "diagnostics: missing or foreign boundary ownership",
        )?;
        require(
            !record.rationale.trim().is_empty(),
            "diagnostics: boundary needs its concrete rationale",
        )?;
        require(
            fingerprint(root, std::slice::from_ref(&record.path))? == record.source_sha256,
            format!("diagnostics: stale boundary {}", record.path),
        )?;
        match record.disposition {
            Disposition::Routing => require(
                record.owner == policy.owner
                    && (record.path == policy.routing_entry
                        || record.path.starts_with(&format!(
                            "{}/",
                            policy.routing_entry.trim_end_matches(".rs")
                        ))
                        || record.path == policy.worker_entry),
                "diagnostics: routing disposition outside its implementation",
            )?,
            Disposition::Storage => require(
                record.owner == policy.storage_owner
                    && (record.path == policy.storage_entry
                        || record.path.starts_with(&format!(
                            "{}/",
                            policy.storage_entry.trim_end_matches(".rs")
                        ))),
                "diagnostics: storage disposition outside its implementation",
            )?,
            Disposition::Developer => require(
                owners[0].layer == "tooling"
                    || path.file_name().is_some_and(|name| name == "build.rs"),
                "diagnostics: runtime sink cannot claim developer ownership",
            )?,
            Disposition::Fixture | Disposition::Capture | Disposition::Upstream => {}
        }
    }
    for entry in [&policy.routing_entry, &policy.storage_entry, &policy.worker_entry] {
        require(
            records.contains_key(entry.as_str()),
            "diagnostics: supported entry requires its reviewed boundary",
        )?;
    }
    let mut used = BTreeSet::new();
    let mut missing = Vec::new();
    for owner in &architecture.owners {
        let dependencies =
            workspace.packages.iter().find(|package| package.name == owner.package).ok_or_else(
                || Failure::Policy("diagnostics: architecture package is missing".into()),
            )?;
        let raw_exports = if owner.package == policy.owner || owner.package == policy.storage_owner
        {
            source::crate_public_raw_exports(
                &resolve(root, &owner.root)?,
                &dependencies.dependencies,
            )?
        } else {
            BTreeSet::new()
        };
        for owned in owner.roots() {
            for path in source_files(&resolve(root, owned)?, EXTENSIONS)? {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|cause| Failure::external("resolve diagnostic source", cause))?
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = read(&path)?;
                let findings = if path.extension().is_some_and(|ext| ext == "rs") {
                    source::rust(&text, &dependencies.dependencies)?
                } else if path.extension().is_some_and(|ext| ext == "html") {
                    source::html(&text)
                } else {
                    source::auxiliary(&text)
                };
                require(
                    !raw_exports.contains(&path)
                        || records.get(relative.as_str()).is_some_and(|record| {
                            matches!(
                                record.disposition,
                                Disposition::Fixture | Disposition::Developer
                            )
                        }),
                    format!("diagnostics: {relative} exports raw or unresolved diagnostic types"),
                )?;
                if !findings.is_empty()
                    || [&policy.routing_entry, &policy.storage_entry, &policy.worker_entry]
                        .contains(&&relative)
                {
                    let Some(record) = records.get(relative.as_str()) else {
                        missing.push(format!("{relative}: {}", findings.join(", ")));
                        continue;
                    };
                    require(
                        !findings.iter().any(|finding| finding == "automatic stderr termination")
                            || matches!(
                                record.disposition,
                                Disposition::Developer | Disposition::Fixture
                            ),
                        format!("diagnostics: {relative} must return an explicit process status"),
                    )?;
                    if matches!(record.disposition, Disposition::Routing | Disposition::Storage)
                        && path.extension().is_some_and(|ext| ext == "rs")
                    {
                        require(
                            !source::public_raw_exports(&text)?,
                            format!("diagnostics: {relative} exports raw diagnostic capabilities"),
                        )?;
                    }
                    used.insert(relative);
                }
            }
        }
    }
    require(
        missing.is_empty(),
        format!("diagnostics: unregistered routes:\n{}", missing.join("\n")),
    )?;
    let unused: Vec<_> = records.keys().filter(|path| !used.contains(**path)).copied().collect();
    require(
        unused.is_empty(),
        format!("diagnostics: remove stale or uncovered boundary records: {}", unused.join(", ")),
    )
}
#[cfg(test)]
mod tests;
