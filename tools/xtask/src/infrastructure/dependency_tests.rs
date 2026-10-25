// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject renamed filesystem dependencies and stale Cargo/npm declaration reviews.
use super::{access, dependencies, tests::fixture};
use crate::{error::Result, fixtures, workspace::Dependency};

/// Declared aliases resolve to canonical libraries without adding fictitious source imports.
#[test]
fn cargo_aliases_are_classified_only_when_authored_source_uses_them() -> Result<()> {
    let dependency = Dependency {
        name: "rustix".into(),
        rename: Some("data_only".into()),
        kind: Some("dev".into()),
        path: None,
    };
    assert!(access::rust("pub fn value()->u8{1}", std::slice::from_ref(&dependency))?.is_empty());
    for source in [
        "fn f(){data_only::fs::open(p);}",
        "use data_only as harmless; fn f(){harmless::fs::open(p);}",
        "macro_rules! f{($lib:ident)=>{$lib::fs::open(p)}}fn g(){f!(data_only)}",
    ] {
        assert!(
            !access::rust(source, std::slice::from_ref(&dependency))?.is_empty(),
            "missed {source}"
        );
    }
    Ok(())
}

/// Optional/inactive/dev dependency identities and bindings remain part of manifest review.
#[test]
fn cargo_dependency_reviews_reject_alias_owner_kind_and_manifest_drift() -> Result<()> {
    let (mut repository, _) = fixture()?;
    fixtures::write(repository.root(), "Cargo.toml", "[workspace]\nmembers=['crates/host']")?;
    fixtures::write(repository.root(), "Cargo.lock", "# Locked fixture declarations")?;
    repository.workspace.packages[0].dependencies.push(Dependency {
        name: "fs2".into(),
        rename: Some("innocent".into()),
        kind: Some("dev".into()),
        path: None,
    });
    let actual = dependencies::inventory(
        repository.root(),
        &repository.architecture,
        &repository.workspace,
    )?;
    assert_eq!(actual.len(), 1);
    assert!(dependencies::check(&actual, &[]).is_err());
    let item = &actual[0];
    let mut reviews = vec![dependencies::Review {
        owner: item.owner.clone(),
        identity: item.identity.clone(),
        binding: item.binding.clone(),
        kind: item.kind.clone(),
        source_sha256: item.source_sha256.clone(),
        rationale: "Fixture deliberately exercises an aliased file-locking dependency".into(),
    }];
    dependencies::check(&actual, &reviews)?;
    reviews[0].binding = "fs2".into();
    assert!(dependencies::check(&actual, &reviews).is_err());
    reviews[0].binding = item.binding.clone();
    reviews[0].kind = "normal".into();
    assert!(dependencies::check(&actual, &reviews).is_err());
    reviews[0].kind = item.kind.clone();
    reviews[0].owner = "foreign".into();
    assert!(dependencies::check(&actual, &reviews).is_err());
    reviews[0].owner = item.owner.clone();
    fixtures::write(repository.root(), "Cargo.lock", "# A changed locked declaration")?;
    let changed = dependencies::inventory(
        repository.root(),
        &repository.architecture,
        &repository.workspace,
    )?;
    assert!(dependencies::check(&changed, &reviews).is_err());
    Ok(())
}

/// npm alias syntax cannot conceal file-capable dependencies or changes to their lockfile.
#[test]
fn npm_aliases_and_exact_manifest_fingerprints_are_reviewed() -> Result<()> {
    let (mut repository, _) = fixture()?;
    fixtures::write(
        repository.root(),
        "installer/package.json",
        r#"{"dependencies":{"safe-data":"npm:fs-extra@11.3.0"}}"#,
    )?;
    fixtures::write(repository.root(), "installer/package-lock.json", "{}")?;
    repository.architecture.owners[0].auxiliary_files.push("installer/package.json".into());
    let actual = dependencies::inventory(
        repository.root(),
        &repository.architecture,
        &repository.workspace,
    )?;
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].identity, "fs-extra");
    assert_eq!(actual[0].binding, "safe-data");
    assert!(dependencies::check(&actual, &[]).is_err());
    let item = &actual[0];
    let review = dependencies::Review {
        owner: item.owner.clone(),
        identity: item.identity.clone(),
        binding: item.binding.clone(),
        kind: item.kind.clone(),
        source_sha256: item.source_sha256.clone(),
        rationale: "Fixture exercises npm aliases".into(),
    };
    dependencies::check(&actual, std::slice::from_ref(&review))?;
    fixtures::write(repository.root(), "installer/package-lock.json", "{\"changed\":true}")?;
    let changed = dependencies::inventory(
        repository.root(),
        &repository.architecture,
        &repository.workspace,
    )?;
    assert!(dependencies::check(&changed, &[review]).is_err());
    Ok(())
}
