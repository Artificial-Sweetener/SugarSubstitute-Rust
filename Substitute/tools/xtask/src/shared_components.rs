// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Govern the exact sibling-component files of the monorepo that this workspace consumes.
//!
//! The workspace is one folder of a larger repository. Its own policy paths never leave the
//! workspace root; a sibling component's files are reachable only through a declaration here,
//! which names the component's owning folder, its exact consumed files and the authored
//! workspace files allowed to reference them. Every authored relative path literal that leaves
//! the workspace must name a declared file or component folder from a declared consumer.

use std::{
    collections::BTreeSet,
    path::{Component as PathPart, Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    error::{Failure, Result, read, require},
    policy::Architecture,
    source::resolve,
};

/// Authored source roots and languages whose relative path literals are inspected.
const ROOTS: &[&str] = &["crates", "tools", "native", "installer"];
const LANGUAGES: &[&str] = &["rs", "mjs", "cjs", "js", "ts", "py", "ps1"];

/// Inventory sibling monorepo components consumed by exact file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Components {
    pub(crate) schema_version: u32,
    pub(crate) components: Vec<Component>,
}

/// Bind one sibling component folder to its owner, consumed files and consumers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Component {
    id: String,
    root: String,
    owner: String,
    files: Vec<String>,
    consumers: Vec<String>,
}

impl Components {
    /// Return whether a component with this id is declared.
    pub(crate) fn has(&self, id: &str) -> bool {
        self.components.iter().any(|component| component.id == id)
    }

    /// Return whether `path` is a declared file of the identified component.
    pub(crate) fn declares(&self, id: &str, path: &str) -> bool {
        self.components
            .iter()
            .any(|component| component.id == id && component.files.iter().any(|file| file == path))
    }
}

/// Resolve a monorepo-relative sibling path without escaping the monorepo or entering links.
///
/// The monorepo root is the workspace root's parent. The path must use forward-slash components,
/// must not point back into the workspace, and no existing component of it may be a symbolic
/// link or junction.
pub(crate) fn resolve_sibling(workspace: &Path, relative: &str) -> Result<PathBuf> {
    require(
        !relative.is_empty()
            && !relative.contains(['\\', ':'])
            && relative.split('/').all(|part| !part.is_empty() && part != "." && part != ".."),
        format!("sibling path must use relative forward-slash components: {relative}"),
    )?;
    let monorepo = workspace
        .parent()
        .ok_or_else(|| Failure::Policy("the workspace has no enclosing monorepo".into()))?;
    let mut candidate = monorepo.to_path_buf();
    for part in relative.split('/') {
        candidate.push(part);
        let metadata = candidate
            .symlink_metadata()
            .map_err(|error| Failure::external(format!("resolve sibling {relative}"), error))?;
        require(
            !metadata.file_type().is_symlink(),
            format!("linked sibling path is not governed: {relative}"),
        )?;
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|error| Failure::external(format!("resolve sibling {relative}"), error))?;
    let canonical_monorepo = monorepo
        .canonicalize()
        .map_err(|error| Failure::external("resolve monorepo root", error))?;
    let canonical_workspace = workspace
        .canonicalize()
        .map_err(|error| Failure::external("resolve workspace root", error))?;
    require(
        canonical.starts_with(&canonical_monorepo) && !canonical.starts_with(&canonical_workspace),
        format!("sibling path must stay in the monorepo and outside the workspace: {relative}"),
    )?;
    Ok(candidate)
}

/// Check declarations and every authored relative path literal that leaves the workspace.
pub(crate) fn check(root: &Path, architecture: &Architecture, policy: &Components) -> Result<()> {
    require(policy.schema_version == 1, "shared components require schema one")?;
    let mut ids = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut consumers = BTreeSet::new();
    for component in &policy.components {
        require(
            !component.id.trim().is_empty()
                && ids.insert(component.id.as_str())
                && !component.owner.trim().is_empty()
                && !component.files.is_empty()
                && !component.consumers.is_empty(),
            "shared components require a unique id, owner, files and consumers",
        )?;
        require(
            resolve_sibling(root, &component.root)?.is_dir(),
            format!("{}: component root must be a directory", component.id),
        )?;
        for file in &component.files {
            require(
                file.starts_with(&format!("{}/", component.root))
                    && files.insert(file.as_str())
                    && resolve_sibling(root, file)?.is_file(),
                format!("{}: duplicate, missing or foreign component file {file}", component.id),
            )?;
        }
        for consumer in &component.consumers {
            require(
                consumers.insert(consumer.as_str()) && resolve(root, consumer)?.is_file(),
                format!("{}: duplicate or missing consumer {consumer}", component.id),
            )?;
        }
    }
    let workspace = lexical(root);
    let monorepo = workspace
        .parent()
        .ok_or_else(|| Failure::Policy("the workspace has no enclosing monorepo".into()))?
        .to_path_buf();
    let mut used_files = BTreeSet::new();
    let mut active_consumers = BTreeSet::new();
    let roots: Vec<String> = ROOTS.iter().map(|root| (*root).to_owned()).collect();
    for path in crate::authored_sources::sources(root, architecture, &roots, LANGUAGES)? {
        let source = read(&path)?;
        let consumer = relative_name(&workspace, &lexical(&path))?;
        for component in &policy.components {
            require(
                !source.contains(component.root.as_str())
                    || component.consumers.contains(&consumer),
                format!("{consumer}: names {} outside its declared consumers", component.root),
            )?;
        }
        for reference in sibling_references(&workspace, &path, &source)? {
            let target = relative_name(&monorepo, &reference)?;
            let component = policy
                .components
                .iter()
                .find(|component| component.files.contains(&target) || component.root == target)
                .ok_or_else(|| {
                    Failure::Policy(format!(
                        "{consumer}: undeclared reference outside the workspace: {target}"
                    ))
                })?;
            require(
                component.consumers.contains(&consumer),
                format!("{consumer}: not a declared consumer of {}", component.id),
            )?;
            active_consumers.insert(consumer.clone());
            if target == component.root {
                used_files.extend(
                    component
                        .files
                        .iter()
                        .filter(|file| {
                            file.rsplit('/').next().is_some_and(|name| {
                                source.contains(&format!("\"{name}\""))
                                    || source.contains(&format!("'{name}'"))
                            })
                        })
                        .cloned(),
                );
            } else {
                used_files.insert(target);
            }
        }
    }
    for component in &policy.components {
        for consumer in &component.consumers {
            require(
                active_consumers.contains(consumer),
                format!("{}: stale consumer {consumer} references no component file", component.id),
            )?;
        }
        for file in &component.files {
            require(
                used_files.contains(file),
                format!("{}: stale component file {file} has no consumer", component.id),
            )?;
        }
    }
    Ok(())
}

/// Collect relative path literals in one authored file that resolve outside the workspace.
///
/// A literal starting with `../` is relative to the file's directory, as `include_str!`,
/// `include!` and `new URL(..., import.meta.url)` resolve it. A literal starting with `/../` is
/// appended to the owning crate directory, as `concat!(env!("CARGO_MANIFEST_DIR"), ...)` builds it.
fn sibling_references(workspace: &Path, file: &Path, source: &str) -> Result<Vec<PathBuf>> {
    let file = lexical(file);
    let directory =
        file.parent().ok_or_else(|| Failure::Policy("authored file has no parent".into()))?;
    let mut references = Vec::new();
    for literal in relative_literals(source) {
        let base = if let Some(stripped) = literal.strip_prefix('/') {
            (crate_directory(workspace, directory)?, stripped)
        } else {
            (directory.to_path_buf(), literal.as_str())
        };
        let target = normalize(&base.0.join(base.1));
        if !target.starts_with(workspace) {
            references.push(target);
        }
    }
    Ok(references)
}

/// Return quoted literals that begin with a parent-directory step.
fn relative_literals(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut literals = Vec::new();
    for (index, quote) in bytes.iter().enumerate() {
        if !matches!(quote, b'"' | b'\'' | b'`') {
            continue;
        }
        let rest = &source[index + 1..];
        let preceding = &source[..index];
        let file_relative = rest.starts_with("../") && opens_file_relative_path(preceding);
        let manifest_relative =
            rest.starts_with("/../") && appends_to_manifest_directory(preceding);
        if !(file_relative || manifest_relative) {
            continue;
        }
        let end = rest
            .find(|character: char| character == char::from(*quote) || character == '\n')
            .unwrap_or(rest.len());
        literals.push(rest[..end].to_owned());
    }
    literals
}

/// Return whether the text before a literal opens a form that resolves it against its own file.
///
/// Only `include_str!`, `include_bytes!`, `include!`, `#[path = ...]` and `new URL(...)` give a
/// `../` literal the authored file's directory as its base; other `../` literals are relative to
/// a runtime directory and cannot reach a sibling component by themselves.
fn opens_file_relative_path(preceding: &str) -> bool {
    let start = preceding.len().saturating_sub(160);
    let window = preceding.get(start..).unwrap_or(preceding);
    let window = window.rsplit(';').next().unwrap_or(window).trim_end();
    ["include_str!(", "include_bytes!(", "include!(", "#[path =", "new URL("]
        .iter()
        .any(|form| window.ends_with(form))
}

/// Return whether a `/../` literal continues a `concat!(env!("CARGO_MANIFEST_DIR"), ...)` path.
fn appends_to_manifest_directory(preceding: &str) -> bool {
    let start = preceding.len().saturating_sub(160);
    let window = preceding.get(start..).unwrap_or(preceding);
    window.rsplit(';').next().unwrap_or(window).contains("env!(\"CARGO_MANIFEST_DIR\")")
}

/// Find the nearest directory holding a `Cargo.toml`, staying inside the workspace.
fn crate_directory(workspace: &Path, start: &Path) -> Result<PathBuf> {
    let mut candidate = start.to_path_buf();
    loop {
        if candidate.join("Cargo.toml").is_file() {
            return Ok(candidate);
        }
        require(
            candidate.starts_with(workspace) && candidate != workspace,
            format!("{}: manifest-relative literal outside a crate", start.display()),
        )?;
        candidate = candidate
            .parent()
            .ok_or_else(|| Failure::Policy("manifest search left the filesystem".into()))?
            .to_path_buf();
    }
}

/// Absolute, lexically normalized form of a path; links are rejected elsewhere.
fn lexical(path: &Path) -> PathBuf {
    normalize(&std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()))
}

/// Remove `.` and `..` components without consulting the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for part in path.components() {
        match part {
            PathPart::CurDir => {}
            PathPart::ParentDir => {
                normal.pop();
            }
            other => normal.push(other.as_os_str()),
        }
    }
    normal
}

/// Spell a path below `base` with forward slashes, as policy files record it.
fn relative_name(base: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(base).map_err(|_| {
        Failure::Policy(format!("{} is outside {}", path.display(), base.display()))
    })?;
    let parts: Vec<_> =
        relative.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Result<crate::fixtures::Monorepo> {
        let fixture = crate::fixtures::Monorepo::new()?;
        crate::fixtures::write(fixture.monorepo(), "Sugar/contracts/schema.json", "{}")?;
        crate::fixtures::write(fixture.monorepo(), "Sugar/contracts/vectors.json", "[]")?;
        Ok(fixture)
    }
    fn policy(files: &[&str], consumers: &[&str]) -> Components {
        Components {
            schema_version: 1,
            components: vec![Component {
                id: "contracts".into(),
                root: "Sugar/contracts".into(),
                owner: "fixture contracts".into(),
                files: files.iter().map(|file| (*file).to_owned()).collect(),
                consumers: consumers.iter().map(|file| (*file).to_owned()).collect(),
            }],
        }
    }

    const CONSUMER: &str = "crates/host/src/tests.rs";

    /// Accept exact declared files and folder references made by declared consumers.
    #[test]
    fn declared_sibling_files_are_accepted_from_declared_consumers() -> Result<()> {
        let fixture = fixture()?;
        crate::fixtures::write(
            fixture.root(),
            CONSUMER,
            "const S: &str = include_str!(\"../../../../Sugar/contracts/schema.json\");\nconst D: &str = concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../../Sugar/contracts/\");\nconst V: &str = \"vectors.json\";\n",
        )?;
        let files = ["Sugar/contracts/schema.json", "Sugar/contracts/vectors.json"];
        check(fixture.root(), &fixture.workspace.architecture, &policy(&files, &[CONSUMER]))
    }

    /// Reject references that no declaration covers, or that come from an undeclared consumer.
    #[test]
    fn undeclared_references_and_consumers_are_rejected() -> Result<()> {
        let fixture = fixture()?;
        crate::fixtures::write(
            fixture.root(),
            CONSUMER,
            "const S: &str = include_str!(\"../../../../Sugar/contracts/schema.json\");\n",
        )?;
        let architecture = &fixture.workspace.architecture;
        assert!(
            check(
                fixture.root(),
                architecture,
                &policy(&["Sugar/contracts/vectors.json"], &[CONSUMER])
            )
            .is_err()
        );
        crate::fixtures::write(fixture.root(), "crates/host/src/other.rs", "// other\n")?;
        assert!(
            check(
                fixture.root(),
                architecture,
                &policy(&["Sugar/contracts/schema.json"], &["crates/host/src/other.rs"])
            )
            .is_err()
        );
        check(fixture.root(), architecture, &policy(&["Sugar/contracts/schema.json"], &[CONSUMER]))
    }

    /// Ignore `../` literals resolved against runtime directories, but not a named component path.
    #[test]
    fn only_file_relative_forms_and_named_component_paths_count_as_references() -> Result<()> {
        let fixture = fixture()?;
        crate::fixtures::write(
            fixture.root(),
            CONSUMER,
            "const S: &str = include_str!(\"../../../../Sugar/contracts/schema.json\");\n",
        )?;
        let tool = "installer/build.mjs";
        crate::fixtures::write(
            fixture.root(),
            tool,
            "const source = path.resolve(root, '../../../outside/tree')\n",
        )?;
        crate::fixtures::write(
            fixture.root(),
            "tools/check.py",
            "for tail in ('/../../../private.cc', '../../../outside'):\n    pass\n",
        )?;
        let architecture = &fixture.workspace.architecture;
        let declared = policy(&["Sugar/contracts/schema.json"], &[CONSUMER]);
        check(fixture.root(), architecture, &declared)?;
        crate::fixtures::write(
            fixture.root(),
            tool,
            "const schema = path.join(monorepo, 'Sugar/contracts/schema.json')\n",
        )?;
        assert!(check(fixture.root(), architecture, &declared).is_err());
        crate::fixtures::write(
            fixture.root(),
            tool,
            "const schema = new URL('../../Sugar/contracts/schema.json', import.meta.url)\n",
        )?;
        assert!(check(fixture.root(), architecture, &declared).is_err());
        Ok(())
    }
    /// Reject stale files and consumers so declarations stay exact.
    #[test]
    fn stale_files_and_consumers_are_rejected() -> Result<()> {
        let fixture = fixture()?;
        crate::fixtures::write(
            fixture.root(),
            CONSUMER,
            "const S: &str = include_str!(\"../../../../Sugar/contracts/schema.json\");\n",
        )?;
        crate::fixtures::write(fixture.root(), "crates/host/src/idle.rs", "// idle\n")?;
        let architecture = &fixture.workspace.architecture;
        let both = ["Sugar/contracts/schema.json", "Sugar/contracts/vectors.json"];
        assert!(check(fixture.root(), architecture, &policy(&both, &[CONSUMER])).is_err());
        assert!(
            check(
                fixture.root(),
                architecture,
                &policy(&["Sugar/contracts/schema.json"], &[CONSUMER, "crates/host/src/idle.rs"])
            )
            .is_err()
        );
        Ok(())
    }

    /// Reject declarations that escape the monorepo, re-enter the workspace or name missing files.
    #[test]
    fn declarations_cannot_escape_reenter_or_name_missing_files() -> Result<()> {
        let fixture = fixture()?;
        let workspace_name = fixture
            .root()
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Failure::Policy("fixture workspace has no name".into()))?;
        let reentry = format!("{workspace_name}/crates/host/src/lib.rs");
        for path in [
            "../outside.json",
            "Sugar/../Sugar/contracts/schema.json",
            "Sugar\\contracts\\schema.json",
            "C:/Sugar/contracts/schema.json",
            reentry.as_str(),
            "Sugar/contracts/missing.json",
        ] {
            assert!(resolve_sibling(fixture.root(), path).is_err(), "accepted {path}");
        }
        let foreign = Components {
            schema_version: 1,
            components: vec![Component {
                id: "contracts".into(),
                root: "Sugar/contracts".into(),
                owner: "fixture".into(),
                files: vec!["Sugar/other/schema.json".into()],
                consumers: vec![CONSUMER.into()],
            }],
        };
        crate::fixtures::write(fixture.root(), CONSUMER, "// consumer\n")?;
        crate::fixtures::write(fixture.monorepo(), "Sugar/other/schema.json", "{}")?;
        assert!(check(fixture.root(), &fixture.workspace.architecture, &foreign).is_err());
        Ok(())
    }

    /// Reject a sibling file reached through a directory link.
    #[test]
    fn linked_sibling_paths_are_rejected() -> Result<()> {
        let fixture = fixture()?;
        let monorepo = fixture.monorepo();

        let link = monorepo.join("Sugar").join("linked");
        if crate::fixtures::directory_link(&monorepo.join("Sugar").join("contracts"), &link)
            .is_err()
        {
            return Err(Failure::Policy("create the directory link fixture".into()));
        }
        assert!(resolve_sibling(fixture.root(), "Sugar/linked/schema.json").is_err());
        resolve_sibling(fixture.root(), "Sugar/contracts/schema.json").map(|_| ())
    }
}
