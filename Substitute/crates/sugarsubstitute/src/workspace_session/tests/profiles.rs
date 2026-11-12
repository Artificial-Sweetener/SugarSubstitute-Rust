// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual native index identities stable and recover previous host records read-only.
use super::*;
use sugarsubstitute_contracts::{
    desktop_session::{InstallationId, ProfileId},
    lifecycle::BackendBindingDigest,
};
fn profile(fixture: &Fixture) -> TestResult<ProfileStore> {
    let scope = checkpoint(1)?.scope;
    Ok(ProfileStore::new(
        fixture.directory.path(),
        InstallationId::new(scope.installation)?,
        ProfileId::new(scope.profile)?,
    )?)
}
fn open(
    fixture: &Fixture,
    store: &ProfileStore,
    digest: u8,
) -> TestResult<Result<OpenedProfile, StoreError>> {
    let job = store.submit_open(
        &fixture.client,
        &fixture.scope,
        fixture.spec()?,
        BackendBindingDigest::from_bytes([digest; 32]),
    )?;
    match fixture.terminal(job)? {
        JobOutcome::Success(profile) => Ok(Ok(profile)),
        JobOutcome::Failure(cause) => Ok(Err(cause)),
        outcome => Err(format!("Profile open stopped: {:?}", outcome.kind()).into()),
    }
}
#[test]
fn profile_reuses_exact_backend_namespace_and_preserves_changed_backend_state() -> TestResult {
    let fixture = Fixture::new()?;
    let store = profile(&fixture)?;
    let first = open(&fixture, &store, 1)??;
    let scope = first.scope.clone();
    let path = first.workspace.directory().to_owned();
    std::fs::create_dir_all(&path)?;
    std::fs::write(path.join("authored.keep"), b"original namespace")?;
    drop(first);
    assert_eq!(open(&fixture, &store, 1)??.scope, scope);
    let changed = open(&fixture, &store, 2)??;
    assert_eq!(changed.scope.workspace, scope.workspace);
    assert_ne!(changed.scope.backend, scope.backend);
    drop(changed);
    assert_eq!(open(&fixture, &store, 1)??.scope, scope);
    assert_eq!(std::fs::read(path.join("authored.keep"))?, b"original namespace");
    Ok(())
}
#[test]
fn profile_recovers_verified_previous_index_without_overwriting_unknown_primary() -> TestResult {
    let fixture = Fixture::new()?;
    let store = profile(&fixture)?;
    let first = open(&fixture, &store, 1)??.scope;
    drop(open(&fixture, &store, 2)??);
    let path = store.directory().join("namespace-index.json");
    let future = b"{\"version\":2,\"future\":\"preserve\"}";
    std::fs::write(&path, future)?;
    let recovered = open(&fixture, &store, 1)??;
    assert_eq!(recovered.scope, first);
    assert!(recovered.protected());
    drop(recovered);
    assert!(matches!(open(&fixture, &store, 3)?, Err(StoreError::Protected)));
    assert_eq!(std::fs::read(&path)?, future);
    Ok(())
}
#[test]
fn profile_inventory_recovery_retains_invalid_primary_and_blocks_new_publication() -> TestResult {
    let fixture = Fixture::new()?;
    let store = profile(&fixture)?;
    let first = open(&fixture, &store, 1)??.scope;
    let path = store.directory().join("asset-inventory.json");
    std::fs::copy(&path, store.directory().join("asset-inventory.previous.json"))?;
    let damaged = b"{\"version\":1,\"interrupted\":";
    std::fs::write(&path, damaged)?;
    let recovered = open(&fixture, &store, 1)??;
    assert_eq!(recovered.scope, first);
    assert!(recovered.protected());
    assert_eq!(std::fs::read(&path)?, damaged);
    Ok(())
}
#[test]
fn profile_appends_to_valid_formatted_inventory_using_exact_original_bytes() -> TestResult {
    use sha2::{Digest, Sha256};
    use sugarsubstitute_contracts::desktop_session::wire::SessionAsset;
    let fixture = Fixture::new()?;
    let store = profile(&fixture)?;
    drop(open(&fixture, &store, 1)??);
    let path = store.directory().join("asset-inventory.json");
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    std::fs::write(&path, serde_json::to_vec_pretty(&value)?)?;
    let mut profile = open(&fixture, &store, 1)??;
    let original = b"verified immutable original";
    let asset = SessionAsset {
        version: 1,
        digest: files::hex(&Sha256::digest(original)),
        bytes: original.len() as u64,
        extension: "png".into(),
        media: "image".into(),
    };
    let expected_path = profile.workspace.asset_path(&asset)?;
    let mut spec = fixture.spec()?;
    spec.reservation_bytes = 64 * 1024 * 1024;
    let job = fixture.client.submit_blocking(&fixture.scope, spec, move |context| {
        profile.retain_asset(&asset, &mut original.as_slice(), &context)
    })?;
    match fixture.terminal(job)? {
        JobOutcome::Success(()) => {}
        JobOutcome::Failure(cause) => return Err(cause.into()),
        outcome => {
            return Err(format!("Formatted inventory append stopped: {:?}", outcome.kind()).into());
        }
    }
    assert_eq!(std::fs::read(expected_path)?, original);
    let inventory: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    assert_eq!(inventory["reservations"].as_array().map(Vec::len), Some(1));
    assert_eq!(inventory["reservations"][0]["verified"], true);
    Ok(())
}
