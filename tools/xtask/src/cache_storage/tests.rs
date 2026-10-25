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
    for directory in ["tools", "native", "installer"] {
        std::fs::create_dir(repository.root().join(directory))
            .map_err(|cause| Failure::external("create cache source fixture", cause))?;
    }
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
        script_boundaries: Vec::new(),
        source_roots: ["crates", "tools", "native", "installer"].map(str::to_owned).to_vec(),
        namespace_reviews: Vec::new(),
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
    for literal in
        ["runtime-cache-v1", "alpha-v1", "cache-v1", "webview-v1", "RUNTIME-CACHE-V1", "AlPhA-V1"]
    {
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

#[test]
fn script_and_native_cache_namespaces_cannot_bypass_the_registered_owner() -> Result<()> {
    for extension in [
        "ts", "mjs", "cjs", "html", "cpp", "h", "py", "ps1", "svg", "cs", "cmake", "gn", "gni",
        "css", "RS", "TS", "CPP", "CS", "CMAKE",
    ] {
        for literal in [
            "runtime-cache-v1",
            "alpha-v1",
            "cache-v1",
            "webview-v1",
            "RUNTIME-CACHE-V1",
            "AlPhA-V1",
        ] {
            let (mut repository, caches, storage) = fixture()?;
            repository.architecture.owners[0].auxiliary_roots.push("native".into());
            fixtures::write(
                repository.root(),
                &format!("native/bypass.{extension}"),
                &format!("const value = \"scope/{literal}/entry\";"),
            )?;
            assert!(
                check(repository.root(), &repository.architecture, &caches, &storage).is_err(),
                "accepted {extension}: {literal}"
            );
        }
    }
    let (mut repository, caches, storage) = fixture()?;
    repository.architecture.owners[0].auxiliary_roots.push("native".into());
    fixtures::write(
        repository.root(),
        "native/CMakeLists.txt",
        "set(STORAGE_ROOT \"runtime-cache-v1\")",
    )?;
    assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    Ok(())
}

#[test]
fn missing_duplicate_or_external_source_coverage_is_rejected() -> Result<()> {
    for variant in 0..4 {
        let (mut repository, caches, mut storage) = fixture()?;
        match variant {
            0 => {
                storage.source_roots.pop();
            }
            1 => storage.source_roots[0] = "tools".into(),
            2 => repository.architecture.owners[0].auxiliary_roots.push("external-worker".into()),
            _ => std::fs::remove_dir(repository.root().join("native"))
                .map_err(|cause| Failure::external("remove fixture source root", cause))?,
        }
        assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    }
    Ok(())
}

#[test]
fn unregistered_tooling_native_and_installer_sources_cannot_construct_known_namespaces()
-> Result<()> {
    for path in [
        "tools/escape.rs",
        "native/escape.CPP",
        "installer/escape.PS1",
        "tools/escape.py",
        "installer/escape.patch",
        "tools/node_modules/escape.rs",
        "installer/unregistered/node_modules/escape.ts",
    ] {
        let (repository, caches, storage) = fixture()?;
        fixtures::write(repository.root(), path, "const VALUE = \"RUNTIME-CACHE-V1/entry\";")?;
        assert!(
            check(repository.root(), &repository.architecture, &caches, &storage).is_err(),
            "accepted {path}"
        );
    }
    Ok(())
}

#[test]
fn namespace_data_reviews_are_exact_tooling_owned_used_and_cannot_authorize_runtime_paths()
-> Result<()> {
    let (mut repository, caches, mut storage) = fixture()?;
    repository.architecture.owners.push(crate::policy::Owner {
        package: "checker".into(),
        root: "tools".into(),
        auxiliary_roots: Vec::new(),
        auxiliary_files: Vec::new(),
        layer: "tooling".into(),
        concern: "Literal rejection input".into(),
    });
    let path = "tools/checker.rs".to_owned();
    fixtures::write(repository.root(), &path, "const INPUT: &str = \"runtime-cache-v1\";")?;
    storage.namespace_reviews.push(coverage::Review {
        source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
        path: path.clone(),
        owner: "checker".into(),
        rationale: "Exact rejection input".into(),
    });
    check(repository.root(), &repository.architecture, &caches, &storage)?;
    storage.namespace_reviews[0].rationale.clear();
    assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    storage.namespace_reviews[0].rationale = "Exact rejection input".into();
    storage.namespace_reviews.push(coverage::Review {
        source_sha256: storage.namespace_reviews[0].source_sha256.clone(),
        path: path.clone(),
        owner: "checker".into(),
        rationale: "Duplicate".into(),
    });
    assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    storage.namespace_reviews.pop();
    storage.namespace_reviews[0].owner = "host".into();
    assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    storage.namespace_reviews[0].owner = "checker".into();
    fixtures::write(repository.root(), &path, "const INPUT: &str = \"offer-v1\";")?;
    assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    fixtures::write(repository.root(), &path, "const INPUT: &str = \"ordinary\";")?;
    storage.namespace_reviews[0].source_sha256 =
        fingerprint(repository.root(), std::slice::from_ref(&path))?;
    assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    let path = "crates/host/src/direct.rs".to_owned();
    fixtures::write(repository.root(), &path, "const INPUT: &str = \"runtime-cache-v1\";")?;
    storage.namespace_reviews[0] = coverage::Review {
        source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
        path,
        owner: "host".into(),
        rationale: "Invalid runtime review".into(),
    };
    assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    Ok(())
}

#[test]
fn installed_dependency_exclusion_requires_matching_manifest_and_lock_ownership() -> Result<()> {
    let (repository, caches, storage) = fixture()?;
    std::fs::create_dir(repository.root().join("installer/node_modules"))
        .map_err(|cause| Failure::external("create dependency fixture", cause))?;
    assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    let manifest = serde_json::json!({"name":"fixture","version":"1.0.0","private":true,
        "dependencies":{"dependency":"1.0.0"}});
    let lock = serde_json::json!({"name":"fixture","lockfileVersion":3,"packages":{"":manifest}});
    fixtures::write(repository.root(), "installer/package.json", &manifest.to_string())?;
    fixtures::write(repository.root(), "installer/package-lock.json", &lock.to_string())?;
    check(repository.root(), &repository.architecture, &caches, &storage)?;
    for invalid in [
        serde_json::json!({}),
        serde_json::json!({"name":"fixture","lockfileVersion":2,"packages":{"":manifest}}),
        serde_json::json!({"name":"foreign","lockfileVersion":3,"packages":{"":manifest}}),
        serde_json::json!({"name":"fixture","lockfileVersion":3,"packages":{"":{
            "name":"fixture","version":"1.0.0","dependencies":{"dependency":"2.0.0"}}}}),
    ] {
        fixtures::write(repository.root(), "installer/package-lock.json", &invalid.to_string())?;
        assert!(check(repository.root(), &repository.architecture, &caches, &storage).is_err());
    }
    Ok(())
}
