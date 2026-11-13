// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Record the reviewed source fingerprints the governance gates found stale.
//!
//! Every governance record that reviews sources stores `source_sha256` beside the sources it
//! reviews: a `sources` list, a `path` with its `lockfile`, or a single `path`. The gates compare
//! that value with [`crate::source::fingerprint`] over exactly those sources, resolving a sibling
//! monorepo component's file through [`crate::shared_components`]. This task computes the same
//! value with the same functions and rewrites only records whose stored value differs, after the
//! person running it has reviewed the changed sources for the concern each record names. It
//! refuses, rewriting nothing, when any reviewed source cannot be resolved, so it never records a
//! fingerprint the gate would not compute.

use std::path::{Path, PathBuf};

use crate::{
    error::{Failure, Result, read, require},
    shared_components::resolve_sibling,
    source::{fingerprint_resolved, resolve},
};

/// The governance policy root, relative to the workspace.
const POLICY_ROOT: &str = "governance";

/// The instruction the gates give when a reviewed fingerprint is stale.
pub(crate) const RESYNC: &str = "review the changed sources for this record's concern, then record them with `cargo xtask fingerprints`";

/// One record whose stored fingerprint differs from its reviewed sources.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Stale {
    /// The policy file, relative to the workspace.
    pub(crate) policy: String,
    /// The reviewed sources, as the record spells them.
    pub(crate) sources: Vec<String>,
    /// The stored fingerprint.
    pub(crate) recorded: String,
    /// The fingerprint the gates compute.
    pub(crate) actual: String,
}

/// Find every stale record under the governance root without changing anything.
///
/// # Errors
/// Rejects unreadable or malformed policy files and sources the gates could not resolve.
#[cfg(test)]
pub(crate) fn stale(root: &Path) -> Result<Vec<Stale>> {
    Ok(review(root)?.into_iter().flat_map(|(_, _, stale)| stale).collect())
}

/// Rewrite every stale record and return what changed, for the reviewer's log.
///
/// Each record is rewritten inside its own table only, so records sharing a placeholder value
/// each receive the fingerprint of their own sources. Nothing is written unless every record in
/// every policy file resolved.
///
/// # Errors
/// Rejects anything [`stale`] rejects, before any file is written, and write failures.
pub(crate) fn record(root: &Path) -> Result<Vec<Stale>> {
    let reviewed = review(root)?;
    let mut changed = Vec::new();
    for (path, text, stale) in reviewed {
        if stale.is_empty() {
            continue;
        }
        std::fs::write(&path, text)
            .map_err(|error| Failure::external(format!("write {}", path.display()), error))?;
        changed.extend(stale);
    }
    Ok(changed)
}

/// Every policy file with its text after recording, and the records that were stale in it.
fn review(root: &Path) -> Result<Vec<(PathBuf, String, Vec<Stale>)>> {
    let mut reviewed = Vec::new();
    for path in policy_files(&root.join(POLICY_ROOT))? {
        let policy = path
            .strip_prefix(root)
            .map_err(|error| Failure::external("relate a policy file to the workspace", error))?
            .to_string_lossy()
            .replace('\\', "/");
        let original = read(&path)?;
        // Reject a malformed policy as a whole before reading its tables one by one.
        toml::from_str::<toml::Value>(&original)
            .map_err(|error| Failure::external(format!("parse {policy}"), error))?;
        let mut text = String::with_capacity(original.len());
        let mut stale = Vec::new();
        for table in tables(&original) {
            text.push_str(&recorded(root, &policy, table, &mut stale)?);
        }
        reviewed.push((path, text, stale));
    }
    Ok(reviewed)
}

/// One table's text, with its fingerprint recorded when it reviews sources and is stale.
fn recorded(root: &Path, policy: &str, table: &str, stale: &mut Vec<Stale>) -> Result<String> {
    let body = table.split_once('\n').filter(|_| header(table)).map_or(table, |(_, body)| body);
    let value: toml::Value = toml::from_str(body)
        .map_err(|error| Failure::external(format!("parse a table of {policy}"), error))?;
    let Some(fields) = value.as_table() else {
        return Ok(table.to_owned());
    };
    let (Some(toml::Value::String(recorded)), Some(sources)) =
        (fields.get("source_sha256"), sources(fields))
    else {
        return Ok(table.to_owned());
    };
    let actual = fingerprint_resolved(&sources, |source| locate(root, source))?;
    if &actual == recorded {
        return Ok(table.to_owned());
    }
    let quoted = format!("\"{recorded}\"");
    require(
        table.matches(&quoted).count() == 1,
        format!("{policy}: the stale value {recorded} must appear once in its table"),
    )?;
    let rewritten = table.replacen(&quoted, &format!("\"{actual}\""), 1);
    stale.push(Stale { policy: policy.to_owned(), sources, recorded: recorded.clone(), actual });
    Ok(rewritten)
}

/// Split policy text at table headers; the first piece holds the root table.
fn tables(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if offset > 0 && header(line) {
            pieces.push(&text[start..offset]);
            start = offset;
        }
        offset += line.len();
    }
    pieces.push(&text[start..]);
    pieces
}

/// Whether `text` starts with a table or array-of-tables header line.
fn header(text: &str) -> bool {
    let line = text.lines().next().unwrap_or("").trim_end();
    line.starts_with('[') && line.ends_with(']') && !line.contains(['"', '\'', ','])
}

/// The sources one record reviews, as the gates select them.
fn sources(table: &toml::map::Map<String, toml::Value>) -> Option<Vec<String>> {
    let text = |key: &str| table.get(key).and_then(toml::Value::as_str).map(str::to_owned);
    if let Some(toml::Value::Array(items)) = table.get("sources") {
        return items.iter().map(|item| item.as_str().map(str::to_owned)).collect();
    }
    let path = text("path")?;
    Some(match text("lockfile") {
        Some(lockfile) => vec![path, lockfile],
        None => vec![path],
    })
}

/// Resolve a workspace source, or a sibling component's file when it lies outside the workspace.
fn locate(root: &Path, relative: &str) -> Result<PathBuf> {
    match resolve(root, relative) {
        Ok(path) => Ok(path),
        Err(local) => resolve_sibling(root, relative).map_err(|_| local),
    }
}

/// Every policy file under `directory`, sorted.
fn policy_files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(directory)
        .map_err(|error| Failure::external(format!("list {}", directory.display()), error))?;
    for entry in entries {
        let entry = entry.map_err(|error| Failure::external("read a policy entry", error))?;
        let kind =
            entry.file_type().map_err(|error| Failure::external("type a policy entry", error))?;
        if kind.is_dir() {
            files.extend(policy_files(&entry.path())?);
        } else if entry.path().extension().is_some_and(|extension| extension == "toml") {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
#[path = "fingerprints_tests.rs"]
mod tests;
