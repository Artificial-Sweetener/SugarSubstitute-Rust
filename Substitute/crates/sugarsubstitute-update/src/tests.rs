// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real signature verification, trust recovery and hostile metadata/payload boundaries.
use super::*;
mod discovery;
mod offer;
mod root;
use crate::fixture::*;
type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn authenticated_targets_verify_streamed_bytes_and_cannot_be_issued_before_refresh() -> TestResult {
    let key = key(1)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    assert!(matches!(trust.target(NAME, NOW), Err(Error::Incomplete)));
    let data = b"a locally signed release descriptor";
    trust.refresh(Set::new(&key, [1; 3], data, EXPIRES)?.metadata(), NOW)?;
    let target = trust.target(NAME, NOW)?;
    assert_eq!(target.name(), NAME);
    assert_eq!(target.length(), data.len() as u64);
    target.require_valid_at(NOW)?;
    assert!(matches!(target.require_valid_at(NOW - 1), Err(Error::ClockRollback)));
    assert!(matches!(target.require_valid_at(EXPIRES), Err(Error::Expired)));
    let mut stream = target.verifier(NOW)?;
    for chunk in data.chunks(3) {
        stream.push(chunk)?;
    }
    let verified = stream.finish(NOW)?;
    assert_eq!(verified.name(), NAME);
    assert_eq!(verified.length(), data.len() as u64);
    Ok(())
}

#[test]
fn resumed_trust_rejects_replay_and_same_version_changed_bytes() -> TestResult {
    let key = key(2)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    let current = Set::new(&key, [2; 3], b"current", EXPIRES)?;
    trust.refresh(current.metadata(), NOW)?;
    let checkpoint = trust.checkpoint()?;
    let mut trust = RepositoryTrust::from_trusted_checkpoint(&checkpoint)?;
    assert!(matches!(trust.target(NAME, NOW), Err(Error::Incomplete)));
    assert!(matches!(
        trust.refresh(Set::new(&key, [1; 3], b"old", EXPIRES)?.metadata(), NOW),
        Err(Error::Rollback)
    ));
    assert_eq!(trust.checkpoint()?, checkpoint);
    assert!(matches!(
        trust.refresh(Set::new(&key, [2; 3], b"changed", EXPIRES)?.metadata(), NOW),
        Err(Error::Equivocation)
    ));
    trust.refresh(current.metadata(), NOW)?;
    assert_eq!(trust.target(NAME, NOW)?.length(), 7);
    Ok(())
}

#[test]
fn newer_timestamp_cannot_rollback_snapshot_or_targets() -> TestResult {
    let key = key(3)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    trust.refresh(Set::new(&key, [2; 3], b"current", EXPIRES)?.metadata(), NOW)?;
    assert!(matches!(
        trust.refresh(Set::new(&key, [3, 1, 1], b"old", EXPIRES)?.metadata(), NOW),
        Err(Error::Rollback)
    ));
    assert!(matches!(
        trust.refresh(Set::new(&key, [4, 3, 1], b"old", EXPIRES)?.metadata(), NOW),
        Err(Error::Rollback)
    ));
    let mut restored = RepositoryTrust::from_trusted_checkpoint(&trust.checkpoint()?)?;
    assert!(matches!(
        restored.refresh(Set::new(&key, [3; 3], b"middle", EXPIRES)?.metadata(), NOW),
        Err(Error::Rollback)
    ));
    restored.refresh(Set::new(&key, [5, 3, 3], b"new", EXPIRES)?.metadata(), NOW)?;
    Ok(())
}

#[test]
fn failed_download_preserves_verified_timestamp_floor_across_restart() -> TestResult {
    let key = key(4)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    let previous = Set::new(&key, [1; 3], b"old", EXPIRES)?;
    trust.refresh(previous.metadata(), NOW)?;
    let mut interrupted = Set::new(&key, [2; 3], b"new", EXPIRES)?;
    interrupted.snapshot.truncate(10);
    assert!(trust.refresh(interrupted.metadata(), NOW).is_err());
    let mut restored = RepositoryTrust::from_trusted_checkpoint(&trust.checkpoint()?)?;
    assert!(matches!(restored.refresh(previous.metadata(), NOW), Err(Error::Rollback)));
    restored.refresh(Set::new(&key, [2; 3], b"new", EXPIRES)?.metadata(), NOW)?;
    Ok(())
}

#[test]
fn expired_metadata_and_clock_rollback_never_authorize_new_targets() -> TestResult {
    let key = key(5)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    let metadata = Set::new(&key, [1; 3], b"payload", NOW + 10)?;
    trust.refresh(metadata.metadata(), NOW)?;
    assert!(matches!(trust.target(NAME, NOW - 1), Err(Error::ClockRollback)));
    assert!(matches!(trust.target(NAME, NOW + 10), Err(Error::Expired)));
    assert!(trust.refresh(metadata.metadata(), NOW + 10).is_err());
    let mut restored = RepositoryTrust::from_trusted_checkpoint(&trust.checkpoint()?)?;
    assert!(matches!(restored.refresh(metadata.metadata(), NOW - 1), Err(Error::ClockRollback)));
    Ok(())
}

#[test]
fn root_rotation_requires_both_authorities_and_contiguous_versions() -> TestResult {
    let old = key(6)?;
    let new = key(7)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&old, 1, None)?)?;
    let checkpoint = trust.checkpoint()?;
    assert!(trust.rotate(&root(&new, 2, None)?).is_err());
    assert!(trust.rotate(&root(&new, 3, Some(&old))?).is_err());
    assert_eq!(trust.checkpoint()?, checkpoint);
    trust.rotate(&root(&new, 2, Some(&old))?)?;
    let mut restored = RepositoryTrust::from_trusted_checkpoint(&trust.checkpoint()?)?;
    assert!(
        restored.refresh(Set::new(&old, [2; 3], b"revoked", EXPIRES)?.metadata(), NOW).is_err()
    );
    restored.refresh(Set::new(&new, [2; 3], b"new", EXPIRES)?.metadata(), NOW)?;
    assert!(restored.rotate(&root(&old, 1, None)?).is_err());
    Ok(())
}

#[test]
fn target_stream_rejects_truncation_tampering_overrun_and_expiration() -> TestResult {
    let key = key(8)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    trust.refresh(Set::new(&key, [1; 3], b"12345", NOW + 10)?.metadata(), NOW)?;
    for bytes in [b"1234".as_slice(), b"xxxxx".as_slice()] {
        let mut stream = trust.target(NAME, NOW)?.verifier(NOW)?;
        stream.push(bytes)?;
        assert!(matches!(stream.finish(NOW), Err(Error::Digest)));
    }
    let mut stream = trust.target(NAME, NOW)?.verifier(NOW)?;
    assert!(matches!(stream.push(b"123456"), Err(Error::Bound)));
    assert!(stream.push(b"12345").is_err());
    assert!(stream.finish(NOW).is_err());
    let mut stream = trust.target(NAME, NOW)?.verifier(NOW)?;
    stream.push(b"12345")?;
    assert!(matches!(stream.finish(NOW + 10), Err(Error::Expired)));
    Ok(())
}

#[test]
fn malformed_unknown_and_oversized_trust_is_rejected_without_fallback() -> TestResult {
    let key = key(9)?;
    let trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    let mut value: serde_json::Value = serde_json::from_slice(&trust.checkpoint()?)?;
    value["schema_version"] = 2.into();
    assert!(RepositoryTrust::from_trusted_checkpoint(&serde_json::to_vec(&value)?).is_err());
    assert!(RepositoryTrust::from_trusted_checkpoint(b"{broken").is_err());
    assert!(matches!(
        RepositoryTrust::bootstrap(&vec![0; MAX_METADATA_BYTES + 1]),
        Err(Error::Bound)
    ));
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    assert!(
        trust
            .refresh(MetadataSet { timestamp: b"unsigned", snapshot: b"", targets: b"" }, NOW)
            .is_err()
    );
    assert!(matches!(trust.target(NAME, NOW), Err(Error::Incomplete)));
    Ok(())
}

#[test]
fn accepted_references_remain_floors_even_when_referenced_metadata_never_arrives() -> TestResult {
    let key = key(10)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    let mut interrupted = Set::new(&key, [1, 10, 10], b"future", EXPIRES)?;
    interrupted.snapshot.clear();
    assert!(trust.refresh(interrupted.metadata(), NOW).is_err());
    let mut trust = RepositoryTrust::from_trusted_checkpoint(&trust.checkpoint()?)?;
    assert!(matches!(
        trust.refresh(Set::new(&key, [2, 9, 9], b"old", EXPIRES)?.metadata(), NOW),
        Err(Error::Rollback)
    ));
    let mut interrupted = Set::new(&key, [3, 10, 20], b"future", EXPIRES)?;
    interrupted.targets.clear();
    assert!(trust.refresh(interrupted.metadata(), NOW).is_err());
    let mut trust = RepositoryTrust::from_trusted_checkpoint(&trust.checkpoint()?)?;
    assert!(matches!(
        trust.refresh(Set::new(&key, [4, 11, 19], b"old", EXPIRES)?.metadata(), NOW),
        Err(Error::Rollback)
    ));
    trust.refresh(Set::new(&key, [5, 12, 20], b"future", EXPIRES)?.metadata(), NOW)?;
    Ok(())
}
