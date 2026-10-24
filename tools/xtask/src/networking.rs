// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enforce one native HTTP adapter and exact external network boundaries across authored roots.
//! Rust imports/macros and Cargo identities are parsed; auxiliary languages receive conservative
//! token inspection. This gate does not prove arbitrary computed calls or dependency behavior.
use crate::{
    capability_source,
    error::{Failure, Result, read, require},
    execution_exports,
    policy::Architecture,
    source::{fingerprint, resolve},
    workspace::Workspace,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
mod access;
mod external;
mod scripts;
const ROOTS: &[&str] = &["crates", "tools", "native", "installer"];
const EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "c", "cc", "cpp", "cxx", "h",
    "hpp", "hxx", "m", "mm", "html", "py", "ps1", "patch",
];
/// Every known transport access requires exact current source and its real boundary owner.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    schema_version: u32,
    owner: String,
    source_roots: Vec<String>,
    records: Vec<Record>,
    dependencies: Vec<Dependency>,
    script_manifests: Vec<scripts::Manifest>,
    external_boundaries: Vec<external::Boundary>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    path: String,
    owner: String,
    source_sha256: String,
    boundary: Boundary,
    rationale: String,
}
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Boundary {
    Implementation,
    ExecutionIpc,
    Fixture,
    Upstream,
    Browser,
    Inspection,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dependency {
    package: String,
    identity: String,
    rationale: String,
}
/// Scan all target/feature/test sources without relying on active compilation or Git tracking.
pub(crate) fn check(
    root: &Path,
    architecture: &Architecture,
    workspace: &Workspace,
    policy: &Policy,
) -> Result<()> {
    require(policy.schema_version == 1, "networking: unsupported schema")?;
    require(
        policy.source_roots.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == ROOTS.iter().copied().collect()
            && policy.source_roots.len() == ROOTS.len(),
        "networking: missing or duplicate required source coverage",
    )?;
    for relative in architecture
        .source_roots
        .iter()
        .map(String::as_str)
        .chain(architecture.owners.iter().flat_map(|owner| owner.roots()))
    {
        require(
            policy.source_roots.iter().any(|source| Path::new(relative).starts_with(source)),
            format!("networking: architecture source root {relative} lacks network coverage"),
        )?;
    }
    let owner = architecture
        .owners
        .iter()
        .find(|owner| owner.package == policy.owner)
        .ok_or_else(|| Failure::Policy("networking: missing adapter owner".into()))?;
    require(
        owner.layer == "adapter"
            && owner.concern == "Bounded native HTTP transport and checked handle custody",
        "networking: require dedicated transport adapter",
    )?;
    external::check(root, &policy.external_boundaries)?;
    let mut dependencies = BTreeSet::new();
    for record in &policy.dependencies {
        require(
            record.package == policy.owner
                || (record.package == "sugarsubstitute-execution" && record.identity == "tokio"),
            "networking: alternate client dependency has no transport authority",
        )?;
        require(
            !record.rationale.trim().is_empty()
                && access::dependency(&record.identity)
                && dependencies.insert((record.package.as_str(), record.identity.as_str())),
            "networking: invalid/duplicate dependency boundary",
        )?;
    }
    let mut used_dependencies = BTreeSet::new();
    for package in &workspace.packages {
        for dependency in &package.dependencies {
            if access::dependency(&dependency.name) {
                let key = (package.name.as_str(), dependency.name.as_str());
                require(
                    dependencies.contains(&key),
                    format!(
                        "networking: {} declares alternate transport dependency {}",
                        package.name, dependency.name
                    ),
                )?;
                used_dependencies.insert(key);
            }
        }
        if package.name == policy.owner || package.name == "sugarsubstitute-execution" {
            let exports = execution_exports::findings_with(
                Path::new(&package.manifest_path)
                    .parent()
                    .ok_or_else(|| Failure::Policy("networking: manifest parent absent".into()))?,
                &package.dependencies,
                access::exported,
            )?;
            require(
                exports.is_empty(),
                format!("networking: raw transport capabilities exported: {}", exports.join(", ")),
            )?;
        }
    }
    require(dependencies == used_dependencies, "networking: stale dependency boundaries")?;
    let mut records = BTreeMap::new();
    for record in &policy.records {
        require(
            records.insert(record.path.as_str(), record).is_none()
                && !record.rationale.trim().is_empty(),
            "networking: duplicate or unexplained source boundary",
        )?;
        require(
            architecture.owners.iter().any(|owner| owner.package == record.owner),
            "networking: unknown source owner",
        )?;
        let path = resolve(root, &record.path)?;
        let belongs = architecture
            .owners
            .iter()
            .find(|owner| owner.roots().any(|r| path.starts_with(root.join(r))));
        let expected = belongs.map(|owner| owner.package.as_str()).unwrap_or(
            if record.path.starts_with("native/") {
                "sugarsubstitute-crash"
            } else {
                "sugarsubstitute-xtask"
            },
        );
        require(record.owner == expected, "networking: foreign source ownership")?;
        require(
            fingerprint(root, std::slice::from_ref(&record.path))? == record.source_sha256,
            format!("networking: stale source boundary {}", record.path),
        )?;
        require(
            (record.boundary == Boundary::Implementation)
                == path.starts_with(root.join(&owner.root)),
            "networking: native implementation outside its sole adapter",
        )?;
        match record.boundary {
            Boundary::ExecutionIpc => require(
                record.owner == "sugarsubstitute-execution"
                    || record.path == "installer/worker/main.ts",
                "networking: foreign authenticated IPC boundary",
            )?,
            Boundary::Upstream => require(
                record.path.starts_with("installer/patches/")
                    || matches!(
                        record.path.as_str(),
                        "installer/prepare-runtime.mjs" | "installer/worker/provision.ts"
                    ),
                "networking: foreign upstream boundary",
            )?,
            Boundary::Inspection => require(
                record.owner == "sugarsubstitute-xtask"
                    && matches!(
                        record.path.as_str(),
                        "tools/xtask/src/networking/access.rs" | "tools/xtask/src/networking.rs"
                    ),
                "networking: foreign inspection-only classifier data",
            )?,
            Boundary::Fixture => require(
                record.path.contains("/tests/")
                    || record.path.ends_with("_tests.rs")
                    || record.path.ends_with("/tests.rs")
                    || record.path.ends_with("/http_fixture.rs")
                    || record.path.starts_with("installer/qualify-"),
                format!(
                    "networking: {} fixture boundary must be qualification/test source",
                    record.path
                ),
            )?,
            Boundary::Browser => require(
                record.owner == "sugarsubstitute-desktop",
                "networking: foreign browser boundary",
            )?,
            _ => {}
        }
    }
    let mut used = BTreeSet::new();
    let mut manifests = BTreeSet::new();
    for relative in &policy.source_roots {
        for path in sources(&resolve(root, relative)?)? {
            let relative = path
                .strip_prefix(root)
                .map_err(|e| Failure::external("resolve network source", e))?
                .to_string_lossy()
                .replace('\\', "/");
            if path.file_name().is_some_and(|name| name == "package.json") {
                manifests.insert(relative);
                continue;
            }
            let source = read(&path)?;
            let findings = if path.extension().is_some_and(|ext| ext == "rs") {
                capability_source::findings_with_literals(
                    &source,
                    access::path,
                    access::fragments,
                    access::ordinary,
                    Some(access::literal),
                )?
            } else {
                access::auxiliary(
                    &source,
                    path.extension().and_then(|ext| ext.to_str()).unwrap_or(""),
                )
            };
            if !findings.is_empty() {
                require(
                    records.contains_key(relative.as_str()),
                    format!(
                        "networking: {relative} bypasses transport ownership: {}",
                        findings.join(", ")
                    ),
                )?;
                if let Some(record) = records.get(relative.as_str()) {
                    require(
                        category_admits(record, &findings),
                        format!(
                            "networking: {} constructs transport outside its declared boundary category: {}",
                            relative,
                            findings.join(", ")
                        ),
                    )?;
                }
                used.insert(relative);
            }
        }
    }
    scripts::check(root, architecture, &policy.script_manifests, &manifests)?;
    require(
        used.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == records.keys().copied().collect(),
        format!(
            "networking: remove stale boundaries without transport access: {}",
            records
                .keys()
                .filter(|path| !used.contains(**path))
                .copied()
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
}
/// Boundary labels cannot authorize native HTTP construction outside its sole adapter.
fn category_admits(record: &Record, findings: &[String]) -> bool {
    match record.boundary {
        Boundary::Implementation | Boundary::Fixture => true,
        Boundary::Inspection => {
            findings.iter().all(|finding| finding == "literal network capability")
        }
        Boundary::Browser => {
            !record.path.ends_with(".rs")
                && findings.iter().all(|finding| {
                    matches!(
                        finding.as_str(),
                        "fetch" | "XMLHttpRequest" | "WebSocket" | "EventSource"
                    )
                })
        }
        Boundary::ExecutionIpc => findings.iter().all(|finding| {
            matches!(finding.as_str(), "std::net" | "tokio::net")
                || finding.starts_with("std::net::")
                || finding.starts_with("tokio::net::")
                || matches!(finding.as_str(), "node:net" | "createConnection" | "electron")
        }),
        Boundary::Upstream => findings.iter().all(|finding| match record.path.as_str() {
            "installer/worker/provision.ts" => {
                matches!(finding.as_str(), "@upstream/lib/download" | "@upstream/lib/fetch")
            }
            "installer/prepare-runtime.mjs" => {
                matches!(finding.as_str(), "@electron/get" | "downloadArtifact")
            }
            _ => matches!(
                finding.as_str(),
                "fetch" | "electron" | "@upstream/lib/download" | "@upstream/lib/fetch"
            ),
        }),
    }
}

/// Dependency installations are external code; generated artifacts must live outside authored roots.
fn sources(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in
        std::fs::read_dir(directory).map_err(|e| Failure::external("scan networking sources", e))?
    {
        let entry = entry.map_err(|e| Failure::external("inspect networking source", e))?;
        if entry.file_name() == "node_modules" {
            continue;
        }
        let kind = entry
            .file_type()
            .map_err(|e| Failure::external("inspect networking source type", e))?;
        require(!kind.is_symlink(), "networking: linked authored source is not governed")?;
        if kind.is_dir() {
            require(
                entry.file_name() != "target",
                "networking: build output in authored source root",
            )?;
            files.extend(sources(&entry.path())?);
        } else if entry.file_name() == "package.json"
            || entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| EXTENSIONS.contains(&e))
        {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}
#[cfg(test)]
mod tests;
