// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind filesystem-capable Cargo/npm dependency identities and aliases to exact manifests.
use crate::{
    error::{Failure, Result, read, require},
    policy::Architecture,
    source::fingerprint,
    workspace::Workspace,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// An exact reviewed dependency declaration is not permission for unreviewed source construction.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Review {
    pub(crate) owner: String,
    pub(crate) identity: String,
    pub(crate) binding: String,
    pub(crate) kind: String,
    pub(crate) source_sha256: String,
    pub(crate) rationale: String,
}

/// Actual locked declaration identity, including inactive targets, dev dependencies and aliases.
#[derive(Serialize)]
pub(crate) struct Access {
    pub(crate) owner: String,
    pub(crate) identity: String,
    pub(crate) binding: String,
    pub(crate) kind: String,
    pub(crate) source_sha256: String,
}

/// Known filesystem libraries require review when added or renamed, independently of source syntax.
fn cargo(identity: &str) -> bool {
    matches!(
        identity,
        "fs2"
            | "fs4"
            | "fs-lock"
            | "fd-lock"
            | "fs_extra"
            | "filetime"
            | "tempfile"
            | "directories"
            | "directories-next"
            | "walkdir"
            | "jwalk"
            | "ignore"
            | "glob"
            | "globwalk"
            | "notify"
            | "notify-debouncer-full"
            | "cap-std"
            | "cap-fs-ext"
            | "rustix"
            | "libc"
            | "winapi"
            | "windows"
            | "windows-sys"
            | "tokio"
            | "async-std"
            | "tiny-skia"
            | "image"
            | "opencv"
    )
}

/// Resolve npm aliases to their real package identity before classifying file-capable libraries.
fn npm(binding: &str, version: &str) -> String {
    let Some(alias) = version.strip_prefix("npm:") else { return binding.into() };
    alias.rfind('@').filter(|index| *index > 0).map_or(alias, |index| &alias[..index]).into()
}

fn node(identity: &str) -> bool {
    matches!(
        identity,
        "fs-extra"
            | "graceful-fs"
            | "fs-jetpack"
            | "memfs"
            | "glob"
            | "globby"
            | "fast-glob"
            | "rimraf"
            | "del"
            | "chokidar"
            | "electron"
            | "original-fs"
    ) || identity.starts_with("@nodelib/fs.")
}

/// Inventory actual declarations; fingerprints include shared pins and committed lockfiles.
pub(super) fn inventory(
    root: &Path,
    architecture: &Architecture,
    workspace: &Workspace,
) -> Result<Vec<Access>> {
    let mut found = Vec::new();
    for package in &workspace.packages {
        if !package.dependencies.iter().any(|dependency| cargo(&dependency.name)) {
            continue;
        }
        let manifest = Path::new(&package.manifest_path)
            .canonicalize()
            .map_err(|cause| Failure::external("resolve storage dependency manifest", cause))?;
        let canonical = root
            .canonicalize()
            .map_err(|cause| Failure::external("resolve storage repository", cause))?;
        let manifest = manifest
            .strip_prefix(canonical)
            .map_err(|cause| Failure::external("resolve storage manifest", cause))?
            .to_string_lossy()
            .replace('\\', "/");
        let sources = vec!["Cargo.toml".into(), "Cargo.lock".into(), manifest];
        let digest = fingerprint(root, &sources)?;
        for dependency in package.dependencies.iter().filter(|dependency| cargo(&dependency.name)) {
            found.push(Access {
                owner: package.name.clone(),
                identity: dependency.name.clone(),
                binding: dependency.rename.as_ref().unwrap_or(&dependency.name).clone(),
                kind: dependency.kind.clone().unwrap_or_else(|| "normal".into()),
                source_sha256: digest.clone(),
            });
        }
    }
    let manifest = root.join("installer/package.json");
    if manifest.is_file() {
        let owners: Vec<_> =
            architecture.owners.iter().filter(|owner| owner.owns(root, &manifest)).collect();
        require(owners.len() == 1, "storage: npm declarations need one exact source owner")?;
        let package: serde_json::Value = serde_json::from_str(&read(&manifest)?)
            .map_err(|cause| Failure::external("parse storage npm declarations", cause))?;
        let mut node_access = Vec::new();
        for kind in ["dependencies", "devDependencies", "optionalDependencies"] {
            if let Some(dependencies) = package[kind].as_object() {
                for (binding, version) in dependencies {
                    let version = version.as_str().ok_or_else(|| {
                        Failure::Policy("storage npm version must be text".into())
                    })?;
                    let identity = npm(binding, version);
                    if node(&identity) {
                        node_access.push(Access {
                            owner: owners[0].package.clone(),
                            identity,
                            binding: binding.clone(),
                            kind: format!("npm:{kind}"),
                            source_sha256: String::new(),
                        });
                    }
                }
            }
        }
        if !node_access.is_empty() {
            let digest = fingerprint(
                root,
                &["installer/package.json".into(), "installer/package-lock.json".into()],
            )?;
            for mut entry in node_access {
                entry.source_sha256 = digest.clone();
                found.push(entry);
            }
        }
    }
    found.sort_by(|a, b| {
        (&a.owner, &a.identity, &a.binding, &a.kind).cmp(&(
            &b.owner,
            &b.identity,
            &b.binding,
            &b.kind,
        ))
    });
    found.dedup_by(|a, b| {
        a.owner == b.owner && a.identity == b.identity && a.binding == b.binding && a.kind == b.kind
    });
    Ok(found)
}

/// Reject unknown, stale, duplicate, aliased or unexplained dependency reviews.
pub(super) fn check(actual: &[Access], reviewed: &[Review]) -> Result<()> {
    require(
        actual.len() == reviewed.len(),
        "storage: dependency reviews must match actual known filesystem declarations",
    )?;
    let mut used = std::collections::BTreeSet::new();
    for review in reviewed {
        let key = (&review.owner, &review.identity, &review.binding, &review.kind);
        require(
            used.insert(key) && !review.rationale.trim().is_empty(),
            "storage: duplicate or unexplained dependency review",
        )?;
        require(
            actual.iter().any(|entry| {
                (&entry.owner, &entry.identity, &entry.binding, &entry.kind) == key
                    && entry.source_sha256 == review.source_sha256
            }),
            format!(
                "storage: stale or foreign filesystem dependency {} for {}",
                review.identity, review.owner
            ),
        )?;
    }
    Ok(())
}
