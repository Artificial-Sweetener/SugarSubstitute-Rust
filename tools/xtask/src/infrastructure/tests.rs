// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject new publication routes, stale dispositions and gaps in auxiliary source coverage.
use super::*;
use crate::fixtures::{self, Repository};

/// Keep migration review tests independent of the workstation clock.
fn check(root: &Path, architecture: &Architecture, policy: &Policy) -> Result<()> {
    let today = time::Date::from_calendar_date(2026, time::Month::October, 23)
        .map_err(|cause| Failure::external("construct policy test date", cause))?;
    super::check(root, architecture, policy, today)
}

fn fixture() -> Result<(Repository, Policy)> {
    let mut repository = Repository::new()?;
    repository.architecture.owners[0].layer = "adapter".into();
    repository.architecture.owners[0].auxiliary_roots.push("worker".into());
    fixtures::write(repository.root(), "worker/main.ts", "export const ready = true")?;
    fixtures::write(
        repository.root(),
        "crates/host/src/storage/native.rs",
        "fn publish() { std::fs::rename(a, b); }",
    )?;
    let path = "crates/host/src/storage/native.rs".to_owned();
    let source_sha256 = fingerprint(repository.root(), std::slice::from_ref(&path))?;
    let policy = Policy {
        schema_version: 1,
        owner: "host".into(),
        implementation_root: "crates/host/src/storage".into(),
        records: vec![Record {
            path,
            owner: "host".into(),
            source_sha256,
            disposition: Disposition::Implementation,
            rationale: "Retained storage publication boundary".into(),
            next_extraction: None,
            review_until: None,
            external_contract: None,
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
        capability_source::findings(source, publication_path, publication_fragments, |_| false)?
            .is_empty()
    );
    Ok(())
}

#[test]
fn every_supported_auxiliary_language_rejects_aliased_and_quoted_publication_routes() -> Result<()>
{
    for extension in EXTENSIONS.iter().filter(|extension| **extension != "rs") {
        for source in [
            "import { rename as harmless } from 'node:fs/promises'; harmless(a,b)",
            "const { renameSync: harmless } = fs; harmless(a,b)",
            "fs['rename'](source, target)",
            "MoveFileExW(source, target, flags)",
        ] {
            let (repository, policy) = fixture()?;
            fixtures::write(repository.root(), &format!("worker/bypass.{extension}"), source)?;
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
            8 => policy.implementation_root = "worker".into(),
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
    let path = "worker/publication.ts".to_owned();
    fixtures::write(repository.root(), &path, "fs.rename(source, target)")?;
    policy.records.push(Record {
        source_sha256: fingerprint(repository.root(), std::slice::from_ref(&path))?,
        path,
        owner: "host".into(),
        disposition: Disposition::Migration,
        rationale: "Migrate existing worker publication".into(),
        next_extraction: Some("Route worker writes through retained storage".into()),
        review_until: Some("2099-01-01".into()),
        external_contract: None,
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

#[test]
fn native_protocol_boundary_requires_its_exact_adapter_source_and_preserved_contract() -> Result<()>
{
    let (mut repository, mut policy) = fixture()?;
    repository.architecture.owners[0].auxiliary_roots.push("native/capture".into());
    let path = "native/capture/database.cc".to_owned();
    fixtures::write(
        repository.root(),
        &path,
        "void dispose() { SetFileInformationByHandle(file, info); }",
    )?;
    let source_sha256 = fingerprint(repository.root(), std::slice::from_ref(&path))?;
    policy.records.push(Record {
        path: path.clone(),
        owner: "host".into(),
        source_sha256: source_sha256.clone(),
        disposition: Disposition::NativeBoundary,
        rationale: "Native SDK database port preserves the external record contract".into(),
        next_extraction: None,
        review_until: None,
        external_contract: Some("capture-database".into()),
    });
    let catalog = format!(
        "schema_version=1\n[[formats]]\nid='capture-database'\nowner='host'\nsources=['{path}']\nsource_sha256='{source_sha256}'\nclassification='protocol'\npreservation='must_preserve'\ncurrent_version=1\naccepted_versions=[1]\ncompatibility='reject'\nmigrations=[]\n"
    );
    fixtures::write(repository.root(), "governance/persistence/catalog.toml", &catalog)?;
    check(repository.root(), &repository.architecture, &policy)?;
    policy.records[1].external_contract = None;
    assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    policy.records[1].external_contract = Some("foreign-database".into());
    assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    policy.records[1].external_contract = Some("capture-database".into());
    repository.architecture.owners[0].layer = "host".into();
    assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    repository.architecture.owners[0].layer = "adapter".into();
    fixtures::write(
        repository.root(),
        "governance/persistence/catalog.toml",
        &catalog.replace("classification='protocol'", "classification='authoritative_state'"),
    )?;
    assert!(check(repository.root(), &repository.architecture, &policy).is_err());
    Ok(())
}
