// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Payload staging holds no trust writer, and its commit detects authority that changed meanwhile.
use super::*;
use crate::setup_storage::{AuthorityChange, TrustFailure};
use sugarsubstitute_contracts::{installation::Candidate, update::Channel};

/// Import and reserve the first signed release, returning the preparation and saved runtime.
fn reserve(
    fixture: &Fixture,
    context: &JobContext,
) -> Result<(SignedPreparation, RuntimePaths), Failure> {
    let preparation = fixture
        .import(SignedPackage::new(&fixture.archive, 1)?, context)?
        .prepare(|| NOW, context)?;
    let runtime = preparation.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
    Ok((preparation, runtime))
}

/// Nothing launchable was published: selection still shows staging and no receipt exists.
fn unpublished(fixture: &Fixture, context: &JobContext) -> TestResult {
    let store = SelectionStore::open(fixture.installation.path(), context)?;
    let record = store.load(context)?.ok_or("Selection absent")?;
    let Some(Candidate::Staging { generation }) = record.candidate() else {
        return Err("Candidate left staging".into());
    };
    let directory = fixture.installation.path().join("releases").join(generation.id.as_str());
    assert!(!directory.join("application-update-v1.record").exists());
    Ok(())
}

#[test]
fn trust_readers_and_writers_proceed_while_the_payload_is_copied() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let (preparation, runtime) = reserve(&fixture, context)?;
        let other = TrustStore::open(fixture.installation.path(), [92; 32], context)?;
        let trust_directory = crate::durable_storage::files::Directory::open(
            &fixture.installation.path().join("state").join("update-trust"),
            false,
        )?;
        let mut copied = false;
        let candidate = preparation.prepared.prepare_staged(
            &preparation.offer.authorization(&preparation.trust),
            || NOW,
            context,
            || {
                // A non-blocking exclusive probe from a separate handle fails if any holder exists.
                let probe = trust_directory.read("writer")?;
                probe.try_lock().map_err(io::Error::from)?;
                drop(probe);
                assert_eq!(other.next_root_version(context).map_err(io::Error::other)?, 2);
                other.subscription(context).map_err(io::Error::other)?;
                let payload = preparation.prepared.stage_verified(&runtime, context)?;
                copied = true;
                Ok(payload)
            },
        )?;
        assert!(copied);
        let store = SelectionStore::open(fixture.installation.path(), context)?;
        let record = store.load(context)?.ok_or("Selection absent")?;
        assert!(matches!(
            record.candidate(),
            Some(Candidate::Prepared { generation }) if generation == candidate.generation()
        ));
        assert!(candidate.directory().join("application-update-v1.record").is_file());
        fixture.preserved()
    })
}

#[test]
fn a_root_rotation_during_staging_fails_the_commit_and_publishes_nothing() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let (preparation, runtime) = reserve(&fixture, context)?;
        let other = TrustStore::open(fixture.installation.path(), [92; 32], context)?;
        let authorization = preparation.offer.authorization(&preparation.trust);
        // Same signing key at a new root version: the old metadata still verifies after rotation.
        let rotated = root(&key(92)?, 2, Some(&key(92)?))?;
        let outcome = preparation.prepared.prepare_staged(
            &authorization,
            || NOW,
            context,
            || {
                other.rotate(&rotated, context).map_err(io::Error::other)?;
                preparation.prepared.stage_verified(&runtime, context)
            },
        );
        assert!(matches!(
            outcome,
            Err(TrustFailure::AuthorityChanged(AuthorityChange::RootRotated))
        ));
        unpublished(&fixture, context)?;
        let retried =
            preparation.prepared.prepare_update(&runtime, &authorization, || NOW, context)?;
        assert!(retried.directory().join("application-update-v1.record").is_file());
        fixture.preserved()
    })
}

#[test]
fn a_subscription_change_during_staging_fails_the_commit_and_publishes_nothing() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let (preparation, runtime) = reserve(&fixture, context)?;
        let other = TrustStore::open(fixture.installation.path(), [92; 32], context)?;
        let authorization = preparation.offer.authorization(&preparation.trust);
        let outcome = preparation.prepared.prepare_staged(
            &authorization,
            || NOW,
            context,
            || {
                let current = other.subscription(context).map_err(io::Error::other)?;
                let canary = other
                    .select_subscription(current, Channel::Canary, context)
                    .map_err(io::Error::other)?;
                other
                    .select_subscription(canary, Channel::Release, context)
                    .map_err(io::Error::other)?;
                preparation.prepared.stage_verified(&runtime, context)
            },
        );
        assert!(matches!(
            outcome,
            Err(TrustFailure::AuthorityChanged(AuthorityChange::SubscriptionChanged))
        ));
        unpublished(&fixture, context)?;
        fixture.preserved()
    })
}
