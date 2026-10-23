// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enforce exact ownership of supported storage publication routes across authored targets.
//!
//! Rust uses parsed imports and macro tokens. Script and native source use conservative
//! identifier inspection, including quoted property names. This does not prove arbitrary
//! computed calls or semantic equivalence; behavioral and native tests remain necessary.
use crate::{
    capability_source,
    error::{Failure, Result, read, require},
    policy::Architecture,
    source::{fingerprint, resolve, source_files},
    structure::check_review,
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

/// Require one implementation and exact finite dispositions for existing publication routes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    pub(crate) schema_version: u32,
    pub(crate) owner: String,
    pub(crate) implementation_root: String,
    pub(crate) records: Vec<Record>,
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
    pub(crate) next_extraction: Option<String>,
    pub(crate) review_until: Option<String>,
}

/// Implementation, adversarial test input, and finite outstanding migration are distinct.
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Disposition {
    Implementation,
    Fixture,
    Migration,
}

/// Inspect all architecture roots, auxiliary languages, tests and inactive platform targets.
pub(crate) fn check(
    root: &Path,
    architecture: &Architecture,
    policy: &Policy,
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
        let matching = architecture
            .owners
            .iter()
            .filter(|owner| owner.roots().any(|relative| path.starts_with(root.join(relative))))
            .collect::<Vec<_>>();
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
    let mut used = BTreeSet::new();
    for owner in &architecture.owners {
        for relative in owner.roots() {
            for path in source_files(&resolve(root, relative)?, EXTENSIONS)? {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|cause| Failure::external("resolve infrastructure source", cause))?
                    .to_string_lossy()
                    .replace('\\', "/");
                let source = read(&path)?;
                let findings = if path.extension().is_some_and(|extension| extension == "rs") {
                    capability_source::findings(
                        &source,
                        publication_path,
                        publication_fragments,
                        |_| false,
                    )?
                } else {
                    lexical_findings(&source)
                };
                if !findings.is_empty() {
                    require(
                        records.contains_key(relative.as_str()),
                        format!(
                            "infrastructure: {relative} bypasses storage publication ownership: {}",
                            findings.join(", ")
                        ),
                    )?;
                    used.insert(relative);
                }
            }
        }
    }
    require(
        used.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == records.keys().copied().collect(),
        "infrastructure: remove stale dispositions without restricted source access",
    )
}

/// Inspect canonical standard-library paths and native linker identities, even after aliasing.
fn publication_path(path: &[String]) -> bool {
    path.starts_with(&["std".into(), "fs".into()])
        && path.last().is_some_and(|part| part == "rename" || part == "*")
        || path.first().is_some_and(|part| part == "libc" || part == "link_name")
            && path.last().is_some_and(|part| part == "rename")
        || path.last().is_some_and(|part| native_publication(part))
}

/// Split declarative macro path fragments cannot conceal these native/standard operations.
fn publication_fragments(names: &BTreeSet<String>) -> bool {
    names.iter().any(|name| native_publication(name))
        || ["fs", "rename"].iter().all(|name| names.contains(*name))
        || ["libc", "rename"].iter().all(|name| names.contains(*name))
}

/// Known native rename/replace and handle-based publication/deletion entry points.
fn native_publication(name: &str) -> bool {
    matches!(
        name,
        "MoveFileA"
            | "MoveFileW"
            | "MoveFileExA"
            | "MoveFileExW"
            | "MoveFileWithProgressA"
            | "MoveFileWithProgressW"
            | "ReplaceFileA"
            | "ReplaceFileW"
            | "SetFileInformationByHandle"
            | "NtSetInformationFile"
            | "renameat"
            | "renameat2"
    )
}

/// Conservative script/native tokens cover aliases, destructuring and quoted properties.
fn lexical_findings(source: &str) -> Vec<String> {
    source
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '$'
        })
        .filter(|name| native_publication(name) || matches!(*name, "rename" | "renameSync"))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests;
