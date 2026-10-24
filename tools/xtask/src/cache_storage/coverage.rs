// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require complete authored-root discovery and exact reviews for checker input namespace data.
use super::*;
use crate::source::{fingerprint, source_files_excluding};
use std::path::PathBuf;

const ROOTS: &[&str] = &["crates", "tools", "native", "installer"];

/// Tooling may contain literal rejection-fixture data only under an exact owned source review.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Review {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) source_sha256: String,
    pub(crate) rationale: String,
}

/// Discover source regardless of Git tracking, active features, architecture registration or case.
pub(super) fn sources(
    root: &Path,
    architecture: &Architecture,
    source_roots: &[String],
) -> Result<BTreeSet<PathBuf>> {
    require(
        source_roots.len() == ROOTS.len()
            && source_roots.iter().map(String::as_str).collect::<BTreeSet<_>>()
                == ROOTS.iter().copied().collect(),
        "cache: missing, duplicate or unsupported authored source coverage",
    )?;
    for relative in architecture
        .source_roots
        .iter()
        .map(String::as_str)
        .chain(architecture.owners.iter().flat_map(|owner| owner.roots()))
    {
        require(
            ROOTS.iter().any(|source| Path::new(relative).starts_with(source)),
            format!("cache: architecture root {relative} lacks source coverage"),
        )?;
    }
    let mut paths = BTreeSet::new();
    let dependencies = root.join("installer/node_modules");
    if dependencies.symlink_metadata().is_ok() {
        dependency_manifest(root)?;
    }
    for relative in source_roots {
        paths.extend(source_files_excluding(
            &resolve(root, relative)?,
            scripts::EXTENSIONS,
            std::slice::from_ref(&dependencies),
        )?);
    }
    Ok(paths)
}

/// The sole installed dependency exclusion requires matching current npm declarations.
fn dependency_manifest(root: &Path) -> Result<()> {
    let decode = |relative| -> Result<serde_json::Value> {
        serde_json::from_str(&read(&resolve(root, relative)?)?)
            .map_err(|cause| Failure::external("decode cache dependency ownership", cause))
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
        "cache: installed dependency root needs matching private npm manifest and schema-three lockfile",
    )
}

/// Validate exact tooling identity and current source before allowing namespace test/checker data.
pub(super) fn reviews<'a>(
    root: &Path,
    architecture: &Architecture,
    records: &'a [Review],
) -> Result<BTreeSet<&'a str>> {
    let mut reviewed = BTreeSet::new();
    for record in records {
        require(
            !record.rationale.trim().is_empty() && reviewed.insert(record.path.as_str()),
            "cache: duplicate or unexplained namespace data review",
        )?;
        let path = resolve(root, &record.path)?;
        let owners: Vec<_> = architecture
            .owners
            .iter()
            .filter(|owner| owner.roots().any(|relative| path.starts_with(root.join(relative))))
            .collect();
        require(
            owners.len() == 1 && owners[0].package == record.owner && owners[0].layer == "tooling",
            "cache: namespace data review requires its exact tooling owner",
        )?;
        require(
            fingerprint(root, std::slice::from_ref(&record.path))? == record.source_sha256,
            format!("cache: stale namespace data review {}", record.path),
        )?;
    }
    Ok(reviewed)
}
