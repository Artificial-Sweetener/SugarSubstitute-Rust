// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual publication keeps signed release history across restarts and interrupted activation.
use super::*;
use crate::setup_storage::TrustFailure;
use sugarsubstitute_contracts::update::{Admission, Rejection};
use sugarsubstitute_update::Error as Verification;

/// A publisher may issue fresh valid metadata containing an older or reused release sequence.
fn offered(
    bytes: &[u8],
    sequence: u64,
    metadata_version: u32,
) -> Result<SignedPackage, Box<dyn std::error::Error + Send + Sync>> {
    let mut signed = SignedPackage::new(bytes, metadata_version)?;
    let mut descriptor: serde_json::Value = serde_json::from_slice(&signed.descriptor)?;
    descriptor["sequence"] = sequence.into();
    signed.descriptor = serde_json::to_vec(&descriptor)?;
    let digest = format!("{:x}", Sha256::digest(bytes));
    signed.metadata = Set::targets(
        &key(92)?,
        [metadata_version; 3],
        &[
            (&signed.request.target_name(), &signed.descriptor),
            (&format!("{digest}.ssupdate"), bytes),
        ],
        EXPIRES,
    )?;
    Ok(signed)
}

#[test]
fn newer_signed_metadata_cannot_downgrade_or_reuse_a_published_sequence_after_reopen() -> TestResult
{
    run(8, |case| {
        case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        let selected = std::fs::read(case.directory.path().join("SugarSubstitute.lnk"))?;
        let history = std::fs::read(case.directory.path().join(".update-trust/releases"))?;
        let reopened = TrustStore::open(case.directory.path(), [92; 32], case.context)?;
        for (signed, expected) in [
            (offered(case.archive, 7, 9)?, Rejection::Rollback),
            (offered(b"different archive", 8, 10)?, Rejection::Identity),
        ] {
            assert!(matches!(reopened.offer(signed.metadata.metadata(), &signed.request,
                &signed.descriptor, NOW, case.context),
                Err(TrustFailure::Verification(Verification::Offer(reason))) if reason == expected));
            assert!(
                matches!(case.prepared.prepare_update(case.runtime, &signed.authorization(&reopened),
                || NOW, case.context),
                Err(TrustFailure::Verification(Verification::Offer(reason))) if reason == expected)
            );
            assert_eq!(std::fs::read(case.directory.path().join("SugarSubstitute.lnk"))?, selected);
            assert_eq!(
                std::fs::read(case.directory.path().join(".update-trust/releases"))?,
                history
            );
        }
        assert!(reopened.refresh(case.signed.metadata.metadata(), NOW, case.context).is_err());
        Ok(())
    })
}

#[test]
fn failed_final_activation_preserves_floor_and_same_package_remains_retryable() -> TestResult {
    run(8, |case| {
        let mut checks = 0;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &case.signed.authorization(case.trust),
                    || {
                        checks += 1;
                        if checks == 3 { EXPIRES } else { NOW }
                    },
                    case.context
                )
                .is_err()
        );
        case.preserved()?;
        let reopened = TrustStore::open(case.directory.path(), [92; 32], case.context)?;
        let older = offered(case.archive, 7, 9)?;
        assert!(matches!(
            reopened.offer(
                older.metadata.metadata(),
                &older.request,
                &older.descriptor,
                NOW,
                case.context
            ),
            Err(TrustFailure::Verification(Verification::Offer(Rejection::Rollback)))
        ));
        let retry = offered(case.archive, 8, 10)?;
        assert_eq!(
            reopened
                .offer(
                    retry.metadata.metadata(),
                    &retry.request,
                    &retry.descriptor,
                    NOW,
                    case.context
                )?
                .admission(),
            Admission::Prepare
        );
        case.preserved()
    })
}

#[test]
fn history_commit_failure_never_publishes_and_releases_exclusion_for_retry() -> TestResult {
    run(8, |case| {
        let trust_directory = Directory::open(&case.directory.path().join(".update-trust"), false)?;
        let pinned = trust_directory.read("releases")?;
        let previous = std::fs::read(trust_directory.path().join("releases"))?;
        assert!(matches!(
            case.prepared.prepare_update(
                case.runtime,
                &case.signed.authorization(case.trust),
                || NOW,
                case.context
            ),
            Err(TrustFailure::Storage(_))
        ));
        case.preserved()?;
        assert_eq!(std::fs::read(trust_directory.path().join("releases"))?, previous);
        drop(pinned);
        let reopened = TrustStore::open(case.directory.path(), [92; 32], case.context)?;
        case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(&reopened),
            || NOW,
            case.context,
        )?;
        assert_ne!(std::fs::read(trust_directory.path().join("releases"))?, previous);
        Ok(())
    })
}

#[test]
fn missing_damaged_or_wrong_record_kind_never_resets_release_history() -> TestResult {
    run(8, |case| {
        let path = case.directory.path().join(".update-trust/releases");
        let original = std::fs::read(&path)?;
        let checkpoint = std::fs::read(case.directory.path().join(".update-trust/checkpoint"))?;
        let foreign = crate::setup_storage::test_worker::installation(99, case.context)?;
        let _other =
            TrustStore::initialize(foreign.path(), [92; 32], &case.signed.root, case.context)?;
        let foreign_history = std::fs::read(foreign.path().join(".update-trust/releases"))?;
        for bytes in [
            None,
            Some(b"damaged".as_slice()),
            Some(checkpoint.as_slice()),
            Some(foreign_history.as_slice()),
        ] {
            match bytes {
                Some(bytes) => std::fs::write(&path, bytes)?,
                None => std::fs::rename(&path, path.with_extension("preserved"))?,
            }
            assert!(
                case.trust
                    .offer(
                        case.signed.metadata.metadata(),
                        &case.signed.request,
                        &case.signed.descriptor,
                        NOW,
                        case.context
                    )
                    .is_err()
            );
            assert!(
                case.prepared
                    .prepare_update(
                        case.runtime,
                        &case.signed.authorization(case.trust),
                        || NOW,
                        case.context
                    )
                    .is_err()
            );
            case.preserved()?;
            match bytes {
                Some(bytes) => {
                    assert_eq!(std::fs::read(&path)?, bytes);
                    std::fs::write(&path, &original)?;
                }
                None => {
                    assert!(!path.exists());
                    std::fs::rename(path.with_extension("preserved"), &path)?;
                }
            }
        }
        Ok(())
    })
}

#[test]
fn discovery_without_publication_does_not_advance_the_package_floor() -> TestResult {
    run(8, |case| {
        let path = case.directory.path().join(".update-trust/releases");
        let original = std::fs::read(&path)?;
        let future = offered(case.archive, 9, 9)?;
        case.trust.offer(
            future.metadata.metadata(),
            &future.request,
            &future.descriptor,
            NOW,
            case.context,
        )?;
        assert_eq!(std::fs::read(&path)?, original);
        let restored = offered(case.archive, 8, 10)?;
        case.prepared.prepare_update(
            case.runtime,
            &restored.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        assert_ne!(std::fs::read(&path)?, original);
        Ok(())
    })
}
