// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The updater floor survives losing any one record and is never set up again without one.
use super::*;
use crate::durable_storage::protected::security::rewrite::{Rewrite, rewrite};
use crate::setup_storage::{
    self as storage, AuthenticatedOffer, SET_ASIDE_FOLDER, UpdaterPackage,
    test_package::{
        repository::SignedPackage,
        updater::{accept_prepared, archive, evidence_version},
    },
    test_worker::{self as worker, TestResult},
};
use sugarsubstitute_contracts::{
    application_launch::updater::CURRENT_UPDATER_CONTRACT,
    update::{Channel, Component},
};
use sugarsubstitute_update::{ENGINE_RELEASE, fixture::NOW};

const RESERVATION: u64 = storage::SIGNED_INSTALLER_RESERVATION_BYTES
    + storage::UPDATER_STAGING_RESERVATION_BYTES
    + storage::RESERVATION_BYTES
    + storage::TRUST_OPERATION_RESERVATION_BYTES;

/// Accept the updater at `sequence` through genuine staging and probe settlement; returns its
/// archive.
fn accept(
    root: &Path,
    sequence: u32,
    context: &JobContext,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let workspace = tempfile::tempdir()?;
    let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
    let (trust, offer, download) =
        evidence_version(root, &bytes, Component::Updater, sequence, context)?;
    let prepared =
        UpdaterPackage::import(root, trust, offer, download, workspace.path(), || NOW, context)?
            .stage(|| NOW, context)?;
    accept_prepared(prepared, context)?;
    Ok(bytes)
}

/// Lose the receipt of the updater accepted at `sequence` and the floors kept beside the trust
/// folder.
fn lose_records(
    root: &Path,
    sequence: u32,
    context: &JobContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let trust = TrustStore::open(root, [92; 32], context)?;
    let accepted = trust.updater_selection(context)?;
    let generation = accepted.accepted().ok_or("The updater was not accepted")?;
    assert_eq!(generation.release.sequence(), u64::from(sequence));
    std::fs::remove_file(
        trust.root.path().join(format!("engine-{}.release", generation.id.as_str())),
    )?;
    drop(trust);
    std::fs::remove_file(root.join("state").join(RECORD))?;
    Ok(())
}

/// Updater sequence floors the trust's release history holds, by channel.
fn updater_floors(
    trust: &TrustStore,
    context: &JobContext,
) -> Result<Vec<(Channel, u64)>, TrustFailure> {
    let history = trust.protected(context, |protection| trust.read_releases(protection))?;
    Ok(history
        .starts()
        .into_iter()
        .filter(|start| start.component == Component::Updater)
        .map(|start| (start.channel, start.sequence))
        .collect())
}

#[test]
fn an_older_updater_stays_refused_after_a_reset_when_its_receipt_was_lost() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let root = worker::installation(191, context)?;
        let accepted = u32::try_from(ENGINE_RELEASE)? + 2;
        let bytes = accept(root.path(), accepted, context)?;
        lose_records(root.path(), accepted, context)?;
        // The next open keeps the floor from the selection and the release history that remain.
        drop(TrustStore::open(root.path(), [92; 32], context)?);
        assert!(root.path().join("state").join(RECORD).is_file(), "no floors were kept");
        rewrite(&root.path().join("state/update-trust/checkpoint"), Rewrite::UsersMayWrite)?;
        let signed = SignedPackage::component(&bytes, accepted, Component::Updater)?;
        let (trust, set_aside) =
            TrustStore::reset_untrusted(root.path(), [92; 32], &signed.root, context)?;
        assert!(set_aside, "untrusted storage was not set aside");
        for channel in [Channel::Release, Channel::Canary] {
            assert!(
                updater_floors(&trust, context)?.contains(&(channel, u64::from(accepted))),
                "fresh trust does not start the {channel:?} updater route at the accepted updater"
            );
        }
        // An older updater offered in metadata newer than any the installation accepted is
        // refused by its route's floor.
        let older = SignedPackage::component_signed_at(
            &bytes,
            accepted - 1,
            Component::Updater,
            accepted + 1,
        )?;
        let refused = AuthenticatedOffer::authenticate(
            &trust,
            older.request,
            older.metadata.metadata(),
            &older.descriptor,
            NOW,
            context,
        );
        let refusal = refused.err().ok_or("an updater older than the accepted one was admitted")?;
        assert!(
            format!("{refusal:?}").contains("Rollback"),
            "refused for another cause: {refusal:?}"
        );
        Ok(())
    })
}

#[test]
fn a_reset_without_any_record_of_accepted_updaters_fails_closed() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let root = worker::installation(192, context)?;
        let accepted = u32::try_from(ENGINE_RELEASE)? + 2;
        let bytes = accept(root.path(), accepted, context)?;
        lose_records(root.path(), accepted, context)?;
        // The selection and release history are only in storage that can no longer be trusted.
        rewrite(&root.path().join("state/update-trust/checkpoint"), Rewrite::UsersMayWrite)?;
        let signed = SignedPackage::component(&bytes, accepted, Component::Updater)?;
        let refused = TrustStore::reset_untrusted(root.path(), [92; 32], &signed.root, context);
        let refusal = refused.err().ok_or("trust was set up again without an updater floor")?;
        assert!(
            format!("{refusal:?}").contains("version history is damaged"),
            "refused for another cause: {refusal:?}"
        );
        // Nothing moved: the storage stays for diagnosis and the next check.
        assert!(root.path().join("state/update-trust/checkpoint").is_file());
        assert!(!root.path().join("user").join(SET_ASIDE_FOLDER).exists());
        Ok(())
    })
}

#[test]
fn kept_floors_that_cannot_be_read_are_written_again_from_the_trust() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let root = worker::installation(193, context)?;
        let accepted = u32::try_from(ENGINE_RELEASE)? + 2;
        let bytes = accept(root.path(), accepted, context)?;
        std::fs::write(root.path().join("state").join(RECORD), b"not the floors that were kept")?;
        // Trust that still opens is the record of what it admitted.
        drop(TrustStore::open(root.path(), [92; 32], context)?);
        rewrite(&root.path().join("state/update-trust/checkpoint"), Rewrite::UsersMayWrite)?;
        let signed = SignedPackage::component(&bytes, accepted, Component::Updater)?;
        let (trust, set_aside) =
            TrustStore::reset_untrusted(root.path(), [92; 32], &signed.root, context)?;
        assert!(set_aside, "untrusted storage was not set aside");
        assert!(
            updater_floors(&trust, context)?.contains(&(Channel::Release, u64::from(accepted))),
            "fresh trust does not start at the accepted updater"
        );
        Ok(())
    })
}
