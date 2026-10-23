// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Invalid repositories demonstrate cache ownership, registry parity and source-bypass rejection.
use super::*;
use crate::{
    fixtures::{self, Repository},
    source::fingerprint,
};

fn fixture() -> Result<(Repository, Caches, Storage)> {
    let repository = Repository::new()?;
    fixtures::write(
        repository.root(),
        "crates/host/src/cache.rs",
        r#"pub const DIRECTORY: &str = "runtime-cache-v1";"#,
    )?;
    fixtures::write(
        repository.root(),
        "crates/host/src/cache/policy.rs",
        r#"enum Namespace { A }
impl Namespace {
    const ALL: [Self; 1] = [Self::A];
    fn directory(self) -> &'static str { match self { Self::A => "alpha-v1" } }
}"#,
    )?;
    let sources = vec!["crates/host/src/cache.rs".into(), "crates/host/src/cache/policy.rs".into()];
    let review = fingerprint(repository.root(), &sources)?;
    let entry = |id: &str, namespace: &str| {
        serde_json::json!({
            "id":id,"owner":"host","sources":sources,"source_sha256":review,
            "namespace":namespace,"storage_schema":1,"compatibility_epoch":1,
            "compatibility_inputs":["feature_owned_key"],"freshness":"Feature verifies bytes",
            "retention":"Bounded idle storage","recovery":"Preserve unknown bytes"
        })
    };
    let caches: Caches = serde_json::from_value(serde_json::json!({"schema_version":1,
        "caches":[entry("storage", "runtime-cache-v1"),entry("feature", "alpha-v1")]}))
    .map_err(|cause| Failure::external("build cache fixture", cause))?;
    let storage = Storage {
        schema_version: 1,
        owner: "host".into(),
        control_cache: "storage".into(),
        entry_source: sources[0].clone(),
        registry_source: sources[1].clone(),
    };
    Ok((repository, caches, storage))
}

#[test]
fn registered_reviewed_storage_accepts_the_matching_actual_namespace_registry() -> Result<()> {
    let (repository, caches, storage) = fixture()?;
    check(repository.root(), &repository.architecture, &caches, &storage)
}

#[test]
fn unknown_storage_schemas_and_missing_or_foreign_control_owners_are_rejected() -> Result<()> {
    for variant in 0..4 {
        let (repository, mut caches, mut storage) = fixture()?;
        match variant {
            0 => storage.schema_version = 2,
            1 => storage.control_cache = "missing".into(),
            2 => storage.owner = "foreign".into(),
            _ => caches.caches[0].sources.retain(|source| source != &storage.registry_source),
        }
        assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    }
    Ok(())
}

#[test]
fn stale_reviews_and_unreviewed_cache_implementation_files_are_rejected() -> Result<()> {
    let (repository, caches, storage) = fixture()?;
    fixtures::write(repository.root(), "crates/host/src/cache/unreviewed.rs", "fn cleanup() {}")?;
    assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    let (repository, caches, storage) = fixture()?;
    fixtures::write(
        repository.root(),
        &storage.entry_source,
        "pub const DIRECTORY: &str = \"other-v1\";",
    )?;
    assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    Ok(())
}

#[test]
fn catalogue_entries_cannot_omit_or_invent_runtime_namespaces() -> Result<()> {
    for variant in 0..2 {
        let (repository, mut caches, storage) = fixture()?;
        if variant == 0 {
            caches.caches.pop();
        } else {
            caches.caches[1].namespace = "foreign-v1".into();
        }
        assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    }
    Ok(())
}

#[test]
fn known_path_bypasses_in_auxiliary_roots_and_macro_arguments_are_rejected() -> Result<()> {
    for literal in ["runtime-cache-v1", "alpha-v1", "cache-v1", "webview-v1"] {
        let (mut repository, caches, storage) = fixture()?;
        repository.architecture.owners[0].auxiliary_roots.push("native".into());
        fixtures::write(
            repository.root(),
            "native/bypass.rs",
            &format!("cache_path!(root, {literal:?});"),
        )?;
        assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    }
    Ok(())
}
