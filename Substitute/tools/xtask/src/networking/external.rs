// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind upstream HTTP sources to the registered component before applying host transport patches.
use super::*;
const COMPONENT: &str = "comfy-desktop-provisioning";
const SOURCES: &[&str] = &[
    "third_party/comfy-desktop/src/main/lib/download.ts",
    "third_party/comfy-desktop/src/main/lib/fetch.ts",
];
/// An upstream implementation is external code, never the application transport implementation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Boundary {
    component: String,
    sources: Vec<String>,
    source_sha256: String,
    rationale: String,
}
/// The third-party catalogue supplies component provenance and exact consumed files.
pub(super) fn check(root: &Path, records: &[Boundary]) -> Result<()> {
    let implementations_present = SOURCES.iter().any(|source| root.join(source).exists());
    let manifest = root.join("third_party/manifest.toml");
    if !manifest.exists() {
        return require(
            records.is_empty() && !implementations_present,
            "networking: upstream registry missing",
        );
    }
    let manifest: toml::Value = toml::from_str(&read(&manifest)?)
        .map_err(|e| Failure::external("parse upstream component registry", e))?;
    require(
        manifest.get("schema_version").and_then(toml::Value::as_integer) == Some(1),
        "networking: unsupported upstream registry schema",
    )?;
    let component =
        manifest.get("components").and_then(toml::Value::as_array).and_then(|components| {
            components
                .iter()
                .find(|entry| entry.get("id").and_then(toml::Value::as_str) == Some(COMPONENT))
        });
    let Some(component) = component else {
        return require(
            records.is_empty() && !implementations_present,
            "networking: unregistered upstream component",
        );
    };
    require(records.len() == 1, "networking: require the actual upstream HTTP source boundary")?;
    let record = &records[0];
    require(
        record.component == COMPONENT && !record.rationale.trim().is_empty(),
        "networking: foreign or unexplained upstream boundary",
    )?;
    require(
        record.sources.len() == SOURCES.len()
            && record.sources.iter().map(String::as_str).collect::<BTreeSet<_>>()
                == SOURCES.iter().copied().collect(),
        "networking: missing or duplicate upstream HTTP implementation coverage",
    )?;
    let files = component
        .get("files")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| Failure::Policy("networking: upstream component files absent".into()))?;
    for path in &record.sources {
        require(
            files.iter().any(|file| file.as_str() == Some(path)),
            "networking: HTTP implementation outside registered upstream component",
        )?;
        let source = read(&resolve(root, path)?)?;
        require(
            access::lexical(&source).iter().any(|name| name == "electron"),
            "networking: stale upstream HTTP implementation",
        )?;
    }
    require(
        fingerprint(root, &record.sources)? == record.source_sha256,
        format!(
            "networking: stale upstream HTTP implementation review; {}",
            crate::fingerprints::RESYNC
        ),
    )
}
#[cfg(test)]
mod tests;
