// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Keep two implementations of one format from drifting by holding both to one vector file.
//!
//! A shared vector file belongs to a sibling component and is read in place by a Rust suite
//! here and a Python suite there. Each suite declares a table naming, for every vector group,
//! the test that drives all of that group's vectors; this check requires both tables to cover
//! exactly the file's groups, every named test to exist, the Rust suite to include the file
//! by its declared path, the Python loader to open it beside itself, and every format version
//! the file declares to equal the persistence catalog's current version for that format.
//! Whether a named test really iterates its group is proven by each suite's own runtime check
//! and by review; the source inspection here cannot prove semantics.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::Deserialize;

use crate::{
    catalog::Persistence,
    error::{Failure, Result, read, require},
    shared_components::{Components, resolve_sibling, sibling_references},
    source::resolve,
};

/// Bind one shared vector file to the suites of both implementations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SharedVectors {
    file: String,
    component: String,
    rust_suites: Vec<String>,
    python_loader: String,
    python_suites: Vec<String>,
    #[serde(default)]
    formats: BTreeMap<String, String>,
}

/// Validate every shared vector declaration against both suites and the catalog.
pub(crate) fn check(root: &Path, components: &Components, catalog: &Persistence) -> Result<()> {
    for shared in &components.shared_vectors {
        check_one(root, components, catalog, shared)?;
    }
    Ok(())
}

fn check_one(
    root: &Path,
    components: &Components,
    catalog: &Persistence,
    shared: &SharedVectors,
) -> Result<()> {
    let label = &shared.file;
    require(
        components.declares(&shared.component, &shared.file)
            && !shared.rust_suites.is_empty()
            && !shared.python_suites.is_empty(),
        format!("{label}: shared vectors need a declared component file and both suites"),
    )?;
    let path = resolve_sibling(root, &shared.file)?;
    let document: serde_json::Value = serde_json::from_str(&read(&path)?)
        .map_err(|error| Failure::external(format!("parse {label}"), error))?;
    let groups = vector_groups(label, &document)?;

    let rust = shared
        .rust_suites
        .iter()
        .map(|suite| Ok((suite.as_str(), read(&resolve(root, suite)?)?)))
        .collect::<Result<Vec<_>>>()?;
    let reads_in_place =
        sibling_references(root, &resolve(root, &shared.rust_suites[0])?, &rust[0].1)?
            .iter()
            .any(|reference| reference == &crate::shared_components::lexical(&path));
    require(
        reads_in_place && rust[0].1.contains("include_str!("),
        format!("{label}: the Rust suite must include the vector file by its declared path"),
    )?;
    require(
        rust[0].1.contains("fn drive(name: &str)"),
        format!("{label}: the Rust suite must define the accessor drive(name: &str)"),
    )?;
    let rust_table = table(&rust, |line| pair(line, "(\"", "\", \"", "\"),"));
    require_driven(label, "Rust", &groups, &rust_table, &rust, rust_drives)?;

    let loader_path = resolve_sibling(root, &shared.python_loader)?;
    let loader = read(&loader_path)?;
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    require(
        loader_path.parent() == path.parent()
            && loader.contains(&format!("with_name(\"{file_name}\")")),
        format!("{label}: the Python loader must open the vector file beside itself"),
    )?;
    require(
        loader.contains("\ndef drive(group: str)"),
        format!("{label}: the Python loader must define the decorator drive(group: str)"),
    )?;
    let module = python_module(&shared.python_loader)?;
    let python = shared
        .python_suites
        .iter()
        .map(|suite| Ok((suite.as_str(), read(&resolve_sibling(root, suite)?)?)))
        .collect::<Result<Vec<_>>>()?;
    for (suite, source) in &python {
        require(
            source.contains(&module),
            format!("{suite}: a Python vector suite must load vectors through {module}"),
        )?;
    }
    let python_table = table(&python, |line| pair(line, "\"", "\": \"", "\","));
    require_driven(label, "Python", &groups, &python_table, &python, python_drives)?;

    for (key, format) in &shared.formats {
        let declared = document
            .get("formats")
            .and_then(|formats| formats.get(key))
            .and_then(|entry| entry.get("version"))
            .and_then(serde_json::Value::as_u64);
        let registered = catalog.current_version(format, &shared.component);
        require(
            declared.is_some() && declared == registered.map(u64::from),
            format!("{label}: format {key} must declare the catalog's current version of {format}"),
        )?;
    }
    Ok(())
}

/// Return the file's vector groups, requiring unique, named vectors in each.
fn vector_groups(label: &str, document: &serde_json::Value) -> Result<BTreeSet<String>> {
    let object = document
        .as_object()
        .ok_or_else(|| Failure::Policy(format!("{label}: vectors must be an object")))?;
    let mut groups = BTreeSet::new();
    for (name, value) in object {
        if matches!(name.as_str(), "schema" | "description") {
            continue;
        }
        let cases =
            value.as_array().or_else(|| value.get("cases").and_then(|cases| cases.as_array()));
        let mut names = BTreeSet::new();
        for case in cases.into_iter().flatten() {
            let case_name =
                case.get("name").and_then(serde_json::Value::as_str).unwrap_or_default();
            require(
                !case_name.is_empty() && names.insert(case_name),
                format!("{label}: group {name} needs unique vector names"),
            )?;
        }
        groups.insert(name.clone());
    }
    Ok(groups)
}

/// Collect `group -> test` pairs from every line of the suites that matches `parse`.
fn table(
    sources: &[(&str, String)],
    parse: impl Fn(&str) -> Option<(String, String)>,
) -> BTreeMap<String, String> {
    sources.iter().flat_map(|(_, source)| source.lines().filter_map(&parse)).collect()
}

/// Read `open group middle test close` from one trimmed line of a suite's group table.
fn pair(line: &str, open: &str, middle: &str, close: &str) -> Option<(String, String)> {
    let body = line.trim().strip_prefix(open)?.strip_suffix(close)?;
    let (group, test) = body.split_once(middle)?;
    let plain = |text: &str| {
        !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    (plain(group) && plain(test)).then(|| (group.to_owned(), test.to_owned()))
}

/// Require the tables to cover exactly `groups` and each named test to drive its own group.
fn require_driven(
    label: &str,
    language: &str,
    groups: &BTreeSet<String>,
    table: &BTreeMap<String, String>,
    sources: &[(&str, String)],
    drives: impl Fn(&str, &str, &str) -> Option<bool>,
) -> Result<()> {
    let driven: BTreeSet<String> = table.keys().cloned().collect();
    require(
        &driven == groups,
        format!("{label}: the {language} suites must drive exactly the vector groups {groups:?}"),
    )?;
    for (group, test) in table {
        let found: Vec<bool> = sources
            .iter()
            .filter_map(|(_, source)| drives(&source.replace("\r\n", "\n"), test, group))
            .collect();
        require(
            !found.is_empty(),
            format!("{label}: the {language} suites name a missing test {test}"),
        )?;
        require(
            found.iter().all(|uses| *uses),
            format!(
                "{label}: the {language} test {test} must take every {group} vector through drive(\"{group}\")"
            ),
        )?;
    }
    Ok(())
}

/// Whether the Rust test `test` defined in `source` reads `group` through the accessor; `None`
/// when `source` does not define it. Its body runs to the next test or the end of the file.
fn rust_drives(source: &str, test: &str, group: &str) -> Option<bool> {
    let start = source.find(&format!("#[test]\nfn {test}("))?;
    let body = &source[start + 1..];
    let body = body.find("\n#[test]").map_or(body, |end| &body[..end]);
    Some(body.contains(&format!("drive(\"{group}\")")))
}

/// Whether the Python test `test` defined in `source` is parametrized over `group` by the
/// accessor; `None` when `source` does not define it. Its decorators are the lines directly
/// above the definition.
fn python_drives(source: &str, test: &str, group: &str) -> Option<bool> {
    let start = source.find(&format!("\ndef {test}("))?;
    let head = &source[..start];
    let decorators = head.rfind("\n\n").map_or(head, |blank| &head[blank..]);
    Some(decorators.lines().any(|line| line.trim() == format!("@drive(\"{group}\")")))
}

/// Return the dotted module name of a Python file relative to its component folder.
fn python_module(loader: &str) -> Result<String> {
    let (_, inner) = loader
        .split_once('/')
        .ok_or_else(|| Failure::Policy(format!("{loader}: loader must sit in a component")))?;
    let module = inner
        .strip_suffix(".py")
        .ok_or_else(|| Failure::Policy(format!("{loader}: loader must be a Python file")))?;
    Ok(module.replace('/', "."))
}

#[cfg(test)]
#[path = "shared_vectors_tests.rs"]
mod tests;
