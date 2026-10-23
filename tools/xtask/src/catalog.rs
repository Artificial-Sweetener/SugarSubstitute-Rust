// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Validate distinct authoritative-format and disposable-cache ownership contracts.

use std::{collections::BTreeSet, path::Path};

use serde::Deserialize;

use crate::{
    error::{Result, require},
    policy::Architecture,
    source::{fingerprint, resolve},
};

/// Inventory implemented persisted formats and cross-version protocols.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Persistence {
    pub(crate) schema_version: u32,
    pub(crate) formats: Vec<Format>,
}

/// Bind schema compatibility to exact source and a must-preserve classification.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Format {
    id: String,
    owner: String,
    sources: Vec<String>,
    source_sha256: String,
    classification: Classification,
    preservation: Preservation,
    current_version: u32,
    accepted_versions: Vec<u32>,
    compatibility: Compatibility,
    migrations: Vec<String>,
}

/// Keep authoritative data and protocols out of cache registration.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Classification {
    AuthoritativeState,
    AuthoredDocument,
    Protocol,
}

/// Make destructive dispositions unrepresentable in authoritative format policy.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Preservation {
    MustPreserve,
}

/// Require explicit migration or rejection behavior for supported versions.
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Compatibility {
    Reject,
    Migrate,
}

/// Inventory independently disposable persistent caches.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Caches {
    pub(crate) schema_version: u32,
    pub(crate) caches: Vec<Cache>,
}

/// Define one cache's source identity, namespace, and complete lifecycle policy.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Cache {
    id: String,
    owner: String,
    sources: Vec<String>,
    source_sha256: String,
    namespace: String,
    storage_schema: u32,
    compatibility_epoch: u32,
    compatibility_inputs: Vec<String>,
    freshness: String,
    retention: String,
    recovery: String,
}

/// Validate exact reviewed sources under a declared runtime owner.
pub(crate) fn reviewed_sources(
    root: &Path,
    policy: &Architecture,
    owner: &str,
    sources: &[String],
    expected: &str,
) -> Result<()> {
    let entry = policy
        .owners
        .iter()
        .find(|entry| entry.package == owner && entry.layer != "tooling")
        .ok_or_else(|| {
            crate::error::Failure::Policy(format!("catalog: unknown runtime owner {owner}"))
        })?;
    for relative in sources {
        require(
            entry.roots().any(|owned| root.join(relative).starts_with(root.join(owned))),
            format!("catalog: {relative} must belong to {owner}"),
        )?;
        resolve(root, relative)?;
    }
    require(
        fingerprint(root, sources)? == expected,
        format!("catalog: {owner} has stale reviewed source fingerprints"),
    )
}

/// Check authoritative preservation and contiguous supported schema migration.
pub(crate) fn check_persistence(
    root: &Path,
    policy: &Architecture,
    catalog: &Persistence,
) -> Result<()> {
    require(catalog.schema_version == 1, "persistence catalog requires schema one")?;
    let mut ids = BTreeSet::new();
    for format in &catalog.formats {
        require(
            !format.id.trim().is_empty() && ids.insert(&format.id),
            "persisted format identifiers must be nonempty and unique",
        )?;
        reviewed_sources(root, policy, &format.owner, &format.sources, &format.source_sha256)?;
        match (&format.classification, &format.preservation) {
            (
                Classification::AuthoritativeState
                | Classification::AuthoredDocument
                | Classification::Protocol,
                Preservation::MustPreserve,
            ) => {}
        }
        check_versions(
            format.current_version,
            &format.accepted_versions,
            format.compatibility == Compatibility::Migrate,
            &format.migrations,
        )?;
    }
    Ok(())
}

/// Enforce unique contiguous versions and exact forward migration chains.
pub(crate) fn check_versions(
    current: u32,
    accepted: &[u32],
    migrate: bool,
    migrations: &[String],
) -> Result<()> {
    require(
        current > 0
            && accepted.last() == Some(&current)
            && accepted.first().is_some_and(|first| *first > 0)
            && accepted.windows(2).all(|pair| pair[0].checked_add(1) == Some(pair[1])),
        "persisted schemas need ordered contiguous accepted versions ending at current",
    )?;
    let expected: Vec<_> =
        accepted.windows(2).map(|pair| format!("{}->{}", pair[0], pair[1])).collect();
    if migrate {
        require(migrations == expected, "declare every contiguous forward migration exactly once")
    } else {
        require(
            accepted == [current] && migrations.is_empty(),
            "reject policy accepts only the current schema and no migrations",
        )
    }
}

/// Validate portable non-overlapping namespaces and explicit cache lifecycle inputs.
pub(crate) fn check_caches(root: &Path, policy: &Architecture, catalog: &Caches) -> Result<()> {
    require(catalog.schema_version == 1, "cache catalog requires schema one")?;
    let mut ids = BTreeSet::new();
    let mut namespaces = Vec::new();
    for cache in &catalog.caches {
        require(
            !cache.id.trim().is_empty() && ids.insert(&cache.id),
            "cache identifiers must be nonempty and unique",
        )?;
        reviewed_sources(root, policy, &cache.owner, &cache.sources, &cache.source_sha256)?;
        namespaces.push(cache.namespace.as_str());
        require(
            cache.storage_schema > 0
                && cache.compatibility_epoch > 0
                && !cache.compatibility_inputs.is_empty()
                && cache
                    .compatibility_inputs
                    .iter()
                    .all(|item| !item.trim().is_empty() && item != "application_version")
                && !cache.freshness.trim().is_empty()
                && !cache.retention.trim().is_empty()
                && !cache.recovery.trim().is_empty(),
            format!(
                "{}: cache requires semantic compatibility inputs, schema, epoch, freshness, retention and recovery",
                cache.id
            ),
        )?;
    }
    check_namespaces(&namespaces)
}

/// Reject traversal, platform-specific names, duplicate namespaces and nesting.
pub(crate) fn check_namespaces(namespaces: &[&str]) -> Result<()> {
    for (index, namespace) in namespaces.iter().enumerate() {
        require(
            !namespace.is_empty()
                && namespace.split('/').all(|part| {
                    !part.is_empty()
                        && part.bytes().all(|byte| {
                            byte.is_ascii_lowercase()
                                || byte.is_ascii_digit()
                                || byte == b'-'
                                || byte == b'_'
                        })
                        && !matches!(part, "con" | "prn" | "aux" | "nul")
                        && !(part.len() == 4
                            && (part.starts_with("com") || part.starts_with("lpt"))
                            && matches!(part.as_bytes()[3], b'1'..=b'9'))
                }),
            format!("invalid portable cache namespace: {namespace}"),
        )?;
        for other in &namespaces[..index] {
            require(
                namespace != other
                    && !namespace.starts_with(&format!("{other}/"))
                    && !other.starts_with(&format!("{namespace}/")),
                format!("overlapping cache namespaces: {namespace}, {other}"),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reject cache registrations without semantic compatibility and lifecycle policy.
    #[test]
    fn cache_catalog_requires_complete_semantic_lifecycle() -> Result<()> {
        let fixture = crate::fixtures::Repository::new()?;
        let sources = vec!["crates/host/src/lib.rs".to_owned()];
        let cache = Cache {
            id: "thumbnails".into(),
            owner: "host".into(),
            source_sha256: fingerprint(fixture.root(), &sources)?,
            sources,
            namespace: "thumbnails".into(),
            storage_schema: 1,
            compatibility_epoch: 1,
            compatibility_inputs: vec!["source_digest".into()],
            freshness: "content identity".into(),
            retention: "bounded bytes".into(),
            recovery: "quarantine and miss".into(),
        };
        let mut catalog = Caches { schema_version: 1, caches: vec![cache] };
        check_caches(fixture.root(), &fixture.architecture, &catalog)?;
        catalog.caches[0].compatibility_inputs = vec!["application_version".into()];
        assert!(check_caches(fixture.root(), &fixture.architecture, &catalog).is_err());
        catalog.caches[0].compatibility_inputs = vec!["source_digest".into()];
        catalog.caches[0].retention.clear();
        assert!(check_caches(fixture.root(), &fixture.architecture, &catalog).is_err());
        Ok(())
    }

    /// Reject stale source reviews and registrations under unknown owners.
    #[test]
    fn catalog_reviews_bind_exact_source_to_runtime_owner() -> Result<()> {
        let fixture = crate::fixtures::Repository::new()?;
        let sources = vec!["crates/host/src/lib.rs".to_owned()];
        let hash = fingerprint(fixture.root(), &sources)?;
        reviewed_sources(fixture.root(), &fixture.architecture, "host", &sources, &hash)?;
        assert!(
            reviewed_sources(fixture.root(), &fixture.architecture, "unknown", &sources, &hash)
                .is_err()
        );
        crate::fixtures::write(fixture.root(), &sources[0], "changed contents")?;
        assert!(
            reviewed_sources(fixture.root(), &fixture.architecture, "host", &sources, &hash)
                .is_err()
        );
        Ok(())
    }

    /// Reject missing intermediate migrations, unsupported histories and duplicates.
    #[test]
    fn schema_versions_require_complete_forward_paths() -> Result<()> {
        check_versions(3, &[1, 2, 3], true, &["1->2".into(), "2->3".into()])?;
        check_versions(1, &[1], false, &[])?;
        assert!(check_versions(3, &[1, 3], true, &["1->3".into()]).is_err());
        assert!(check_versions(3, &[1, 2, 3], true, &["2->3".into()]).is_err());
        assert!(check_versions(2, &[1, 2], false, &[]).is_err());
        assert!(check_versions(1, &[1, 1], true, &[]).is_err());
        Ok(())
    }

    /// Prove cache namespaces cannot alias each other or escape their storage root.
    #[test]
    fn namespaces_are_portable_and_independently_owned() -> Result<()> {
        check_namespaces(&["model-metadata", "thumbnails"])?;
        for invalid in [
            vec!["a", "a/b"],
            vec!["a", "a"],
            vec!["../a"],
            vec!["C:/a"],
            vec!["a\\b"],
            vec!["con"],
            vec!["lpt1"],
            vec!["A"],
            vec![""],
        ] {
            assert!(check_namespaces(&invalid).is_err(), "accepted {invalid:?}");
        }
        Ok(())
    }

    /// Prevent authoritative formats from being declared disposable caches.
    #[test]
    fn authoritative_preservation_rejects_disposable_classifications() {
        assert!(toml::from_str::<Persistence>("schema_version = 1\n[[formats]]\nid = 'x'\nowner = 'host'\nsources = []\nsource_sha256 = ''\nclassification = 'persistent_cache'\npreservation = 'disposable'\ncurrent_version = 1\naccepted_versions = [1]\ncompatibility = 'reject'\nmigrations = []").is_err());
    }
}
