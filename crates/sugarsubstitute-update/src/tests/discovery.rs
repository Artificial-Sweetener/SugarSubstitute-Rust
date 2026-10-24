// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticated fetch references survive interrupted discovery without granting target authority.
use super::*;

#[test]
fn discovery_references_are_authenticated_durable_and_never_target_authority() -> TestResult {
    let key = key(31)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    let set = Set::new(&key, [3, 7, 9], b"new", EXPIRES)?;
    assert_eq!(trust.next_root_version()?, 2);
    assert_eq!(trust.snapshot_version(&set.timestamp, NOW)?, 7);
    assert!(matches!(trust.target(NAME, NOW), Err(Error::Incomplete)));
    let mut trust = RepositoryTrust::from_trusted_checkpoint(&trust.checkpoint()?)?;
    let old = Set::new(&key, [4, 6, 8], b"old", EXPIRES)?;
    assert!(matches!(trust.snapshot_version(&old.timestamp, NOW), Err(Error::Rollback)));
    assert_eq!(trust.targets_version(&set.timestamp, &set.snapshot, NOW)?, 9);
    assert!(matches!(trust.target(NAME, NOW), Err(Error::Incomplete)));
    let mut trust = RepositoryTrust::from_trusted_checkpoint(&trust.checkpoint()?)?;
    let old = Set::new(&key, [4, 8, 8], b"old", EXPIRES)?;
    assert!(matches!(
        trust.targets_version(&old.timestamp, &old.snapshot, NOW),
        Err(Error::Rollback)
    ));
    let current = Set::new(&key, [5, 8, 9], b"new", EXPIRES)?;
    trust.refresh(current.metadata(), NOW)?;
    assert!(trust.target(NAME, NOW).is_ok());
    assert_eq!(trust.snapshot_version(&current.timestamp, NOW)?, 8);
    assert!(matches!(trust.target(NAME, NOW), Err(Error::Incomplete)));
    Ok(())
}

#[test]
fn invalid_discovery_clears_prior_authority_without_advancing_unsigned_references() -> TestResult {
    let key = key(32)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    let set = Set::new(&key, [3, 7, 9], b"new", EXPIRES)?;
    trust.refresh(set.metadata(), NOW)?;
    let checkpoint = trust.checkpoint()?;
    assert!(trust.snapshot_version(b"unsigned", NOW).is_err());
    assert_eq!(trust.checkpoint()?, checkpoint);
    assert!(matches!(trust.target(NAME, NOW), Err(Error::Incomplete)));
    assert!(trust.targets_version(&set.timestamp, b"unsigned", NOW).is_err());
    assert_eq!(trust.checkpoint()?, checkpoint);
    assert!(trust.snapshot_version(&set.timestamp, EXPIRES).is_err());
    assert!(matches!(trust.target(NAME, NOW), Err(Error::Incomplete)));
    Ok(())
}
