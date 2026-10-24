// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enforce reviewed filesystem construction, methods, dependencies and raw public capabilities.
//!
//! Rust uses parsed imports and macro tokens. Script and native source use conservative
//! identifier inspection, including quoted property names. This does not prove arbitrary
//! computed calls or semantic equivalence; behavioral and native tests remain necessary.
use crate::workspace::Workspace;
use crate::{
    error::{Failure, Result, require},
    policy::Architecture,
    source::{fingerprint, resolve},
    structure::check_review,
};
use serde::Deserialize;
mod access;
mod dependencies;
mod inventory;
pub(crate) use inventory::collect as inventory;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

const EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "c", "cc", "cpp", "cxx", "h",
    "hpp", "hxx", "m", "mm", "html", "py", "ps1", "patch", "cs", "cmake", "gn", "gni", "css",
];

/// Require one implementation and exact finite dispositions for existing publication routes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    pub(crate) schema_version: u32,
    pub(crate) owner: String,
    pub(crate) implementation_root: String,
    pub(crate) records: Vec<Record>,
    pub(crate) source_roots: Vec<String>,
    pub(crate) dependencies: Vec<dependencies::Review>,
}

/// Bind each permitted low-level source to its actual architecture owner and current bytes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) source_sha256: String,
    pub(crate) disposition: Disposition,
    pub(crate) rationale: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) boundary: Option<Boundary>,
    pub(crate) consumer: Option<Consumer>,
    pub(crate) next_extraction: Option<String>,
    pub(crate) review_until: Option<String>,
}

/// Implementation, adversarial test input, and finite outstanding migration are distinct.
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Disposition {
    Implementation,
    Fixture,
    Tooling,
    Boundary,
    Consumer,
    Migration,
}

/// Reviewed external security/lifecycle contexts retain their own native authority.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Boundary {
    NativeCustody,
    NativeIpc,
    Entropy,
    ExternalRuntime,
    NativeInjection,
    Build,
}

/// Method names shared with filesystem APIs require an exact reviewed typed receiver contract.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Consumer {
    ExecutionMetadata,
    StorageEntries,
    MemoryMetadata,
    ProtocolMetadata,
}

/// Inspect all architecture roots, auxiliary languages, tests and inactive platform targets.
pub(crate) fn check(
    root: &Path,
    architecture: &Architecture,
    policy: &Policy,
    workspace: &Workspace,
    today: time::Date,
) -> Result<()> {
    require(policy.schema_version == 1, "infrastructure: unsupported policy schema")?;
    let implementation = resolve(root, &policy.implementation_root)?;
    require(implementation.is_dir(), "infrastructure: storage implementation must be a directory")?;
    let owner = architecture
        .owners
        .iter()
        .find(|owner| owner.package == policy.owner)
        .ok_or_else(|| Failure::Policy("infrastructure: unknown storage owner".into()))?;
    require(
        owner.layer == "adapter" && implementation.starts_with(root.join(&owner.root)),
        "infrastructure: storage implementation must belong to its registered adapter",
    )?;
    let mut records = BTreeMap::new();
    for record in &policy.records {
        require(
            records.insert(record.path.as_str(), record).is_none(),
            "infrastructure: duplicate source disposition",
        )?;
        require(
            !record.rationale.trim().is_empty(),
            "infrastructure: disposition needs a rationale",
        )?;
        let path = resolve(root, &record.path)?;
        let matching =
            architecture.owners.iter().filter(|owner| owner.owns(root, &path)).collect::<Vec<_>>();
        require(
            matching.len() == 1 && matching[0].package == record.owner,
            format!("infrastructure: {} has foreign or missing ownership", record.path),
        )?;
        require(
            fingerprint(root, std::slice::from_ref(&record.path))? == record.source_sha256,
            format!("infrastructure: stale source disposition {}", record.path),
        )?;
        require(
            (record.disposition == Disposition::Implementation)
                == path.starts_with(&implementation),
            format!("infrastructure: {} has an invalid implementation disposition", record.path),
        )?;
        require(
            !record.capabilities.is_empty()
                && record.capabilities.iter().collect::<BTreeSet<_>>().len()
                    == record.capabilities.len(),
            "storage: source review needs distinct actual capability identities",
        )?;
        require(
            (record.disposition == Disposition::Boundary) == record.boundary.is_some(),
            "storage: only an external boundary has a native security/lifecycle classification",
        )?;
        require(
            (record.disposition == Disposition::Consumer) == record.consumer.is_some(),
            "storage: only a typed consumer review names a receiver contract",
        )?;
        if let Some(consumer) = &record.consumer {
            let allowed = match consumer {
                Consumer::ExecutionMetadata
                | Consumer::StorageEntries
                | Consumer::MemoryMetadata
                | Consumer::ProtocolMetadata => "method metadata",
            };
            require(
                record.capabilities.iter().all(|capability| capability == allowed),
                "storage: a typed metadata consumer cannot grant raw filesystem construction",
            )?;
        }
        if record.disposition == Disposition::Tooling {
            require(
                matching[0].layer == "tooling",
                "storage: tooling review cannot exempt runtime source",
            )?;
        }
        if matches!(record.boundary, Some(Boundary::NativeInjection)) {
            require(
                record
                    .capabilities
                    .iter()
                    .all(|capability| matches!(capability.as_str(), "readFile" | "writeFile")),
                "storage: native injection cannot acquire an independent filesystem implementation",
            )?;
        }
        if let Some(boundary) = &record.boundary {
            let layer = matching[0].layer.as_str();
            let permitted = match boundary {
                Boundary::NativeCustody | Boundary::NativeIpc | Boundary::Entropy => {
                    layer == "adapter"
                }
                Boundary::NativeInjection | Boundary::ExternalRuntime => {
                    matches!(layer, "host" | "adapter")
                }
                Boundary::Build => {
                    layer == "tooling"
                        || path.file_name().is_some_and(|name| name == "build.rs")
                        || path
                            .extension()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| matches!(name, "gn" | "gni" | "cmake"))
                        || path
                            .strip_prefix(root.join(&matching[0].root))
                            .is_ok_and(|relative| relative.starts_with("build"))
                }
            };
            require(permitted, "storage: external boundary has foreign lifecycle ownership")?;
        }
        if record.disposition == Disposition::Migration {
            require(
                record.next_extraction.as_ref().is_some_and(|value| !value.trim().is_empty()),
                "infrastructure: migration requires its next extraction",
            )?;
            check_review(record.review_until.as_deref().unwrap_or(""), today)?;
        } else {
            require(
                record.next_extraction.is_none() && record.review_until.is_none(),
                "infrastructure: only migration dispositions have extraction and expiry",
            )?;
        }
    }
    let actual = inventory(root, architecture, workspace, &policy.source_roots)?;
    dependencies::check(&actual.dependencies, &policy.dependencies)?;
    require(
        actual.sources.len() == records.len(),
        "storage: source reviews must match actual restricted access; remove stale reviews",
    )?;
    for source in actual.sources {
        let record = records.get(source.path.as_str()).ok_or_else(|| {
            Failure::Policy(format!(
                "storage: {} bypasses filesystem ownership: {}",
                source.path,
                source.capabilities.iter().cloned().collect::<Vec<_>>().join(", ")
            ))
        })?;
        require(
            source.owner == record.owner
                && source.source_sha256 == record.source_sha256
                && source.capabilities == record.capabilities.iter().cloned().collect(),
            format!("storage: {} has stale or expanded restricted capabilities", source.path),
        )?;
        if source.capabilities.iter().any(|capability| capability.starts_with("export ")) {
            let native_interop =
                matches!(&record.boundary, Some(Boundary::NativeCustody | Boundary::NativeIpc))
                    && source
                        .capabilities
                        .iter()
                        .filter(|capability| capability.starts_with("export "))
                        .all(|capability| {
                            ["::OwnedHandle", "::BorrowedHandle", "::OwnedFd", "::BorrowedFd"]
                                .iter()
                                .any(|name| capability.ends_with(name))
                        });
            require(
                matches!(record.disposition, Disposition::Migration | Disposition::Fixture)
                    || native_interop,
                format!("storage: {} exports an unretired raw filesystem capability", source.path),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod access_tests;
#[cfg(test)]
mod dependency_tests;
#[cfg(test)]
mod tests;
