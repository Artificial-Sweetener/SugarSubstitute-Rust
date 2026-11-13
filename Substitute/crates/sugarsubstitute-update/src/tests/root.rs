// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Threshold and expiry transitions use actual signing keys, including revoked-key attempts.
use super::*;
use tuf::{crypto::PrivateKey, metadata::RootMetadataBuilder, pouf::Pouf1};

#[test]
fn two_of_three_root_threshold_cannot_be_satisfied_by_one_or_duplicate_signatures() -> TestResult {
    let a = key(31)?;
    let b = key(32)?;
    let c = key(33)?;
    let replacement = key(34)?;
    let mut signed = RootMetadataBuilder::new()
        .version(1)
        .expires(time(EXPIRES)?)
        .consistent_snapshot(true)
        .root_threshold(2)
        .root_key(a.public().clone())
        .root_key(b.public().clone())
        .root_key(c.public().clone())
        .timestamp_key(a.public().clone())
        .snapshot_key(a.public().clone())
        .targets_key(a.public().clone())
        .signed::<Pouf1>(&a)?;
    signed.add_signature(&a)?;
    assert!(RepositoryTrust::bootstrap(signed.to_raw()?.as_bytes()).is_err());
    let mut duplicate: serde_json::Value = serde_json::from_slice(signed.to_raw()?.as_bytes())?;
    let signatures = duplicate["signatures"].as_array_mut().ok_or("Missing fixture signatures")?;
    signatures.push(signatures.first().ok_or("Missing fixture signature")?.clone());
    assert!(RepositoryTrust::bootstrap(&serde_json::to_vec(&duplicate)?).is_err());
    signed.add_signature(&b)?;
    let mut trust = RepositoryTrust::bootstrap(signed.to_raw()?.as_bytes())?;
    assert!(trust.rotate(&root(&replacement, 2, Some(&a))?).is_err());
    let mut next = RootMetadataBuilder::new()
        .version(2)
        .expires(time(EXPIRES)?)
        .consistent_snapshot(true)
        .root_key(replacement.public().clone())
        .timestamp_key(replacement.public().clone())
        .snapshot_key(replacement.public().clone())
        .targets_key(replacement.public().clone())
        .signed::<Pouf1>(&replacement)?;
    next.add_signature(&a)?;
    next.add_signature(&b)?;
    trust.rotate(next.to_raw()?.as_bytes())?;
    trust.refresh(Set::new(&replacement, [1; 3], b"new", EXPIRES)?.metadata(), NOW)?;
    Ok(())
}

#[test]
fn expired_root_can_rotate_but_cannot_authorize_an_offer_before_rotation() -> TestResult {
    let a = key(41)?;
    let b = key(42)?;
    let old = RootMetadataBuilder::new()
        .version(1)
        .expires(time(NOW - 1)?)
        .consistent_snapshot(true)
        .root_key(a.public().clone())
        .timestamp_key(a.public().clone())
        .snapshot_key(a.public().clone())
        .targets_key(a.public().clone())
        .signed::<Pouf1>(&a)?
        .to_raw()?;
    let mut trust = RepositoryTrust::bootstrap(old.as_bytes())?;
    assert!(matches!(
        trust.refresh(Set::new(&a, [1; 3], b"old", EXPIRES)?.metadata(), NOW),
        Err(Error::Expired)
    ));
    trust.rotate(&root(&b, 2, Some(&a))?)?;
    trust.refresh(Set::new(&b, [1; 3], b"new", EXPIRES)?.metadata(), NOW)?;
    Ok(())
}

#[test]
fn changing_signers_does_not_erase_previously_accepted_release_metadata_versions() -> TestResult {
    let a = key(51)?;
    let b = key(52)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&a, 1, None)?)?;
    trust.refresh(Set::new(&a, [5; 3], b"accepted", EXPIRES)?.metadata(), NOW)?;
    trust.rotate(&root(&b, 2, Some(&a))?)?;
    assert!(matches!(
        trust.refresh(Set::new(&b, [1; 3], b"rollback", EXPIRES)?.metadata(), NOW),
        Err(Error::Rollback)
    ));
    trust.refresh(Set::new(&b, [6; 3], b"new", EXPIRES)?.metadata(), NOW)?;
    Ok(())
}

#[test]
fn root_version_reports_the_trusted_root_and_advances_with_each_rotation() -> TestResult {
    let signing = key(45)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&signing, 1, None)?)?;
    assert_eq!(trust.root_version()?, 1);
    trust.rotate(&root(&signing, 2, Some(&signing))?)?;
    assert_eq!(trust.root_version()?, 2);
    Ok(())
}
