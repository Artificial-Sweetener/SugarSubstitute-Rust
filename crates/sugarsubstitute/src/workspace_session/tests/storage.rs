// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual files across recovery, stale publication and failed asset admission.
use super::*;
#[test]
fn oversized_primary_recovers_previous_read_only_without_hashing_or_replacing_original()
-> TestResult {
    let fixture = Fixture::new()?;
    fixture.save(checkpoint(1)?)??;
    fixture.save(checkpoint(2)?)??;
    let path = fixture.path(CURRENT_CHECKPOINT);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(MAX_CHECKPOINT_BYTES as u64 + 1)?;
    let restored = fixture.load()?;
    assert!(restored.protected);
    assert_eq!(restored.checkpoint.as_ref().map(|(_, checkpoint)| checkpoint.revision), Some(1));
    drop(restored);
    assert!(matches!(fixture.save(checkpoint(3)?)?, Err(StoreError::Protected)));
    assert_eq!(std::fs::metadata(path)?.len(), MAX_CHECKPOINT_BYTES as u64 + 1);
    Ok(())
}
#[test]
fn workspace_store_recovers_previous_when_primary_is_missing() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.save(checkpoint(1)?)??;
    let first = std::fs::read(fixture.path(CURRENT_CHECKPOINT))?;
    fixture.save(checkpoint(2)?)??;
    assert_eq!(std::fs::read(fixture.path(PREVIOUS_CHECKPOINT))?, first);
    std::fs::remove_file(fixture.path(CURRENT_CHECKPOINT))?;
    let restored = fixture.load()?;
    assert_eq!(
        restored.checkpoint.map(|(file, value)| (file, value.revision)),
        Some((CheckpointFile::Previous, 1))
    );
    assert!(!restored.protected);
    Ok(())
}
#[test]
fn corrupt_primary_uses_previous_and_preserves_original_bytes_before_replacement() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.save(checkpoint(1)?)??;
    fixture.save(checkpoint(2)?)??;
    let rejected = b"{\"version\":1,\"interrupted\":";
    std::fs::write(fixture.path(CURRENT_CHECKPOINT), rejected)?;
    std::fs::write(fixture.path("unknown.txt"), b"keep")?;
    let restored = fixture.load()?;
    assert_eq!(restored.checkpoint.as_ref().map(|(_, value)| value.revision), Some(1));
    assert_eq!(restored.rejected.len(), 1);
    drop(restored);
    let receipt = fixture.save(checkpoint(3)?)??;
    assert_eq!(receipt.evidence.len(), 1);
    assert_eq!(std::fs::read(&receipt.evidence[0])?, rejected);
    assert_eq!(std::fs::read(fixture.path("unknown.txt"))?, b"keep");
    assert_eq!(fixture.load()?.checkpoint.map(|(_, value)| value.revision), Some(3));
    Ok(())
}
#[test]
fn newer_primary_keeps_original_and_inhibits_empty_autosave_despite_usable_previous() -> TestResult
{
    let fixture = Fixture::new()?;
    fixture.save(checkpoint(1)?)??;
    fixture.save(checkpoint(2)?)??;
    let future = b"{\"version\":2,\"future-state\":\"must survive\"}";
    std::fs::write(fixture.path(CURRENT_CHECKPOINT), future)?;
    let restored = fixture.load()?;
    assert!(restored.protected);
    assert_eq!(restored.checkpoint.as_ref().map(|(_, value)| value.revision), Some(1));
    drop(restored);
    assert!(matches!(fixture.save(checkpoint(3)?)?, Err(StoreError::Protected)));
    assert_eq!(std::fs::read(fixture.path(CURRENT_CHECKPOINT))?, future);
    Ok(())
}
#[test]
fn foreign_scope_and_nonincreasing_publication_cannot_replace_good_state() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.save(checkpoint(2)?)??;
    let bytes = std::fs::read(fixture.path(CURRENT_CHECKPOINT))?;
    assert!(fixture.save(checkpoint(1)?)?.is_err());
    let mut foreign = checkpoint(3)?;
    foreign.scope.profile = "other".into();
    assert!(matches!(fixture.save(foreign)?, Err(StoreError::Protected)));
    assert_eq!(std::fs::read(fixture.path(CURRENT_CHECKPOINT))?, bytes);
    Ok(())
}
#[test]
fn retained_asset_is_immutable_and_wrong_source_never_publishes() -> TestResult {
    use sha2::{Digest, Sha256};
    use sugarsubstitute_contracts::desktop_session::wire::SessionAsset;
    let fixture = Fixture::new()?;
    let content = b"immutable encoded final";
    let asset = SessionAsset {
        version: 1,
        digest: files::hex(&Sha256::digest(content)),
        bytes: content.len() as u64,
        extension: "png".into(),
        media: "image".into(),
    };
    let directory = fixture.directory.path().join("assets");
    let copied = directory.clone();
    let expected = asset.clone();
    let job = fixture.client.submit_blocking(&fixture.scope, fixture.spec()?, move |context| {
        assets::retain(&copied, &expected, &mut content.as_slice(), &context)
    })?;
    assert!(matches!(fixture.terminal(job)?, JobOutcome::Success(())));
    let path = directory.join(format!("{}.png", asset.digest));
    assert_eq!(std::fs::read(&path)?, content);
    std::fs::write(&path, b"changed")?;
    let job = fixture.client.submit_blocking(&fixture.scope, fixture.spec()?, move |context| {
        assets::retain(&directory, &asset, &mut content.as_slice(), &context)
    })?;
    assert!(matches!(fixture.terminal(job)?, JobOutcome::Failure(_)));
    assert_eq!(std::fs::read(&path)?, b"changed");
    Ok(())
}
