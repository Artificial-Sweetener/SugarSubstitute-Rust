// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Decode strict, versioned policy documents without accepting unknown fields.

use std::path::Path;

use serde::{Deserialize, de::DeserializeOwned};

use crate::error::{Failure, Result, read, require};

/// Load a schema-one policy and preserve parsing context.
pub(crate) fn load<T: DeserializeOwned>(root: &Path, relative: &str) -> Result<T> {
    let source = read(&root.join(relative))?;
    let value: toml::Value = toml::from_str(&source)
        .map_err(|error| Failure::external(format!("parse {relative}"), error))?;
    require(
        value.get("schema_version").and_then(toml::Value::as_integer) == Some(1),
        format!("{relative}: unsupported or missing schema_version"),
    )?;
    toml::from_str(&source).map_err(|error| Failure::external(format!("decode {relative}"), error))
}

/// Declare authored source scope and authoritative crate layers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Architecture {
    pub(crate) schema_version: u32,
    pub(crate) soft_lines: usize,
    pub(crate) hard_lines: usize,
    pub(crate) source_roots: Vec<String>,
    pub(crate) layers: Vec<String>,
    pub(crate) owners: Vec<Owner>,
}

/// Bind one crate's path, dependency layer, and reason to change.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Owner {
    pub(crate) package: String,
    pub(crate) root: String,
    pub(crate) layer: String,
    pub(crate) concern: String,
}

/// Hold an explicitly empty or reviewed registry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Records<T> {
    pub(crate) schema_version: u32,
    pub(crate) records: Vec<T>,
}

/// Record current mixed ownership and a concrete extraction.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Debt {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) responsibilities: Vec<String>,
    pub(crate) tracking_issue: String,
    pub(crate) next_extraction: String,
    pub(crate) source_sha256: String,
    pub(crate) review_until: String,
}

/// Limit one cohesive exception or shrinking remediation to reviewed source.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Waiver {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) kind: WaiverKind,
    pub(crate) rationale: String,
    pub(crate) source_sha256: String,
    pub(crate) review_until: String,
    pub(crate) line_limit: usize,
    pub(crate) debt_id: Option<String>,
    pub(crate) next_limit: Option<usize>,
}

/// Restrict exceptions to supported ownership dispositions.
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WaiverKind {
    Structural,
    Remediation,
}

/// Define test ownership and required nonempty inventories.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Testing {
    pub(crate) schema_version: u32,
    pub(crate) platforms: Vec<String>,
    pub(crate) owners: Vec<TestOwner>,
}

/// Assign one package's tests to its behavioral concern.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TestOwner {
    pub(crate) package: String,
    pub(crate) root: String,
    pub(crate) concern: String,
    pub(crate) minimum_tests: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reject future schemas and fields rather than silently discarding policy.
    #[test]
    fn loading_rejects_unknown_schemas_and_fields() -> Result<()> {
        let temporary =
            tempfile::tempdir().map_err(|error| Failure::external("create fixture", error))?;
        for source in
            ["schema_version = 2\nrecords = []", "schema_version = 1\nrecords = []\nunknown = true"]
        {
            std::fs::write(temporary.path().join("policy.toml"), source)
                .map_err(|error| Failure::external("write fixture", error))?;
            assert!(load::<Records<Debt>>(temporary.path(), "policy.toml").is_err());
        }
        Ok(())
    }
}
