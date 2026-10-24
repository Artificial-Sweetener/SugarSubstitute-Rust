// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject new publication routes, stale dispositions and gaps in auxiliary source coverage.
use super::*;
use crate::{
    capability_source,
    fixtures::{self, Repository},
};

/// Keep migration review tests independent of the workstation clock.
pub(super) fn check(root: &Path, architecture: &Architecture, policy: &Policy) -> Result<()> {
    let today = time::Date::from_calendar_date(2026, time::Month::October, 23)
        .map_err(|cause| Failure::external("construct policy test date", cause))?;
    let owners: Vec<_> = architecture.owners.iter().map(|owner| serde_json::json!({"name": owner.package, "id": owner.package, "manifest_path": root.join(&owner.root).join("Cargo.toml"), "dependencies": []})).collect();
    let workspace: Workspace = serde_json::from_value(serde_json::json!({"workspace_members": architecture.owners.iter().map(|owner| &owner.package).collect::<Vec<_>>(), "packages": owners}))
        .map_err(|cause| Failure::external("construct storage fixture workspace", cause))?;
    super::check(root, architecture, policy, &workspace, today)
}

pub(super) fn fixture() -> Result<(Repository, Policy)> {
    let mut repository = Repository::new()?;
    repository.architecture.owners[0].layer = "adapter".into();
    repository.architecture.owners[0].auxiliary_roots.push("installer/worker".into());
    fixtures::write(repository.root(), "installer/worker/main.ts", "export const ready = true")?;
    fixtures::write(
        repository.root(),
        "crates/host/src/storage/native.rs",
        "fn publish() { std::fs::rename(a, b); }",
    )?;
    for directory in ["tools", "native"] {
        fixtures::write(repository.root(), &format!("{directory}/.keep"), "")?;
    }
    let path = "crates/host/src/storage/native.rs".to_owned();
    let source_sha256 = fingerprint(repository.root(), std::slice::from_ref(&path))?;
    let policy = Policy {
        schema_version: 1,
        owner: "host".into(),
        implementation_root: "crates/host/src/storage".into(),
        source_roots: ["crates", "tools", "native", "installer"].map(String::from).into(),
        dependencies: Vec::new(),
        records: vec![Record {
            path,
            owner: "host".into(),
            source_sha256,
            disposition: Disposition::Implementation,
            rationale: "Retained storage publication boundary".into(),
            capabilities: vec!["std::fs::rename".into()],
            boundary: None,
            consumer: None,
            next_extraction: None,
            review_until: None,
        }],
    };
    Ok((repository, policy))
}

#[test]
fn rust_aliases_macros_native_linker_names_and_inactive_targets_cannot_hide_publication()
-> Result<()> {
    let bypasses = [
        "use std::fs::rename as innocent; fn f() { innocent(a, b); }",
        "pub use std::fs as disk; fn f() { disk::rename(a, b); }",
        "use std::fs::{self}; fn f() { fs::rename(a, b); }",
        "use std::{fs::{self}}; fn f() { fs::rename(a, b); }",
        "use std::fs::*; fn f() { rename(a, b); }",
        "use libc::rename as publish; fn f() { publish(a, b); }",
        "macro_rules! bypass { () => { std::fs::rename(a, b) } }",
        "use windows::Win32::Storage::FileSystem::MoveFileExW as harmless;",
        "#[cfg(unix)] fn other_platform() { std::fs::rename(a,b); }",
        "#[cfg(test)] mod tests { fn input() { std::fs::rename(a,b); } }",
        "unsafe extern \"system\" { #[link_name = \"MoveFileExW\"] fn harmless(); }",
        "unsafe extern \"C\" { #[link_name = \"rename\"] fn harmless(); }",
        "macro_rules! paths { ($root:ident) => { $root::fs::rename(a,b) } }",
        "macro_rules! paths { ($root:ident, $module:ident, $operation:ident) => { $root::$module::$operation(a,b) } } fn f() { paths!(std, fs, rename); }",
    ];
    for source in bypasses {
        let (repository, policy) = fixture()?;
        fixtures::write(repository.root(), "crates/host/src/bypass.rs", source)?;
        assert!(
            check(repository.root(), &repository.architecture, &policy).is_err(),
            "accepted {source}"
        );
    }
    Ok(())
}

#[test]
fn ordinary_rust_fields_and_literals_do_not_grant_native_publication_access() -> Result<()> {
    let source = "struct Alias { rename: String } fn f(value: Alias) { let rename = value.rename; let diagnostic = \"std::fs::rename\"; }";
    assert!(
        capability_source::findings(source, access::restricted, access::fragments, |_| false)?
            .is_empty()
    );
    Ok(())
}

#[test]
fn every_supported_auxiliary_language_rejects_aliased_and_quoted_publication_routes() -> Result<()>
{
    for extension in EXTENSIONS.iter().filter(|extension| **extension != "rs") {
        let mut sources = vec!["MoveFileExW(source, target, flags)"];
        if matches!(
            *extension,
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "html" | "patch"
        ) {
            sources.extend([
                "import { rename as harmless } from 'node:fs/promises'; harmless(a,b)",
                "const { renameSync: harmless } = fs; harmless(a,b)",
                "fs['rename'](source, target)",
            ]);
        }
        for source in sources {
            let (repository, policy) = fixture()?;
            fixtures::write(
                repository.root(),
                &format!("installer/worker/bypass.{extension}"),
                source,
            )?;
            assert!(
                check(repository.root(), &repository.architecture, &policy).is_err(),
                "accepted {extension}: {source}"
            );
        }
    }
    Ok(())
}

#[test]
fn exact_dispositions_reject_unknown_owners_stale_bytes_and_unsupported_schemas() -> Result<()> {
    for variant in 0..12 {
        let (repository, mut policy) = fixture()?;
        check(repository.root(), &repository.architecture, &policy)?;
        match variant {
            0 => policy.schema_version = 2,
            1 => policy.owner = "missing".into(),
            2 => policy.records[0].owner = "foreign".into(),
            3 => policy.records[0].source_sha256 = "stale".into(),
            4 => policy.records[0].rationale.clear(),
            5 => policy.records[0].disposition = Disposition::Fixture,
            6 => policy.records[0].next_extraction = Some("irrelevant".into()),
            7 => fixtures::write(repository.root(), &policy.records[0].path, "fn unused() {}")?,
            8 => policy.implementation_root = "installer/worker".into(),
            9 => {
                let (_, mut duplicate) = fixture()?;
                policy.records.push(duplicate.records.remove(0));
            }
            10 => policy.records[0].path = "../foreign.rs".into(),
            _ => policy.implementation_root = "crates/host/src/storage/native.rs".into(),
        }
        assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    }
    Ok(())
}

#[test]
fn migration_records_require_finite_review_extraction_and_live_restricted_access() -> Result<()> {
    let (repository, mut policy) = fixture()?;
    let path = "installer/worker/publication.ts".to_owned();
    fixtures::write(repository.root(), &path, "fs.rename(source, target)")?;
    policy.records.push(Record {
        source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
        path,
        owner: "host".into(),
        disposition: Disposition::Migration,
        rationale: "Migrate existing worker publication".into(),
        capabilities: access::script("fs.rename(source, target)", "ts"),
        boundary: None,
        consumer: None,
        next_extraction: Some("Route worker writes through retained storage".into()),
        review_until: Some("2099-01-01".into()),
    });
    check(repository.root(), &repository.architecture, &policy)?;
    policy.records[1].next_extraction = None;
    assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    policy.records[1].next_extraction = Some("Extract".into());
    policy.records[1].review_until = Some("2000-01-01".into());
    assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    policy.records[1].review_until = Some("2099-01-01".into());
    fixtures::write(repository.root(), &policy.records[1].path, "export const ready = true")?;
    policy.records[1].source_sha256 =
        fingerprint(repository.root(), std::slice::from_ref(&policy.records[1].path))?;
    assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    Ok(())
}

#[test]
fn strict_policy_loading_rejects_unknown_fields_and_missing_required_source_coverage() -> Result<()>
{
    let repository = Repository::new()?;
    for source in [
        "schema_version = 2",
        "schema_version = 1\nowner = 'host'\nimplementation_root = 'crates/host/src'\nrecords = []\nunknown = true",
        "schema_version = 1\nowner = 'host'\nrecords = []",
    ] {
        fixtures::write(repository.root(), "policy.toml", source)?;
        assert!(crate::policy::load::<Policy>(repository.root(), "policy.toml").is_err());
    }
    Ok(())
}
