// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replace obsolete pending releases through real signatures, setup custody and native publication.
use super::*;
use crate::setup_storage::test_package::maintenance::accept_pending;
use sugarsubstitute_contracts::update::Channel;

mod interruption;

/// Complete an inert signed release; no health is implied and no executable is started.
fn complete(
    fixture: &Fixture,
    version: u32,
    context: &JobContext,
) -> Result<SignedCandidate, Failure> {
    let prepared = fixture
        .import(SignedPackage::new(&fixture.archive, version)?, context)?
        .prepare(|| NOW, context)?;
    let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
    Ok(prepared.complete(&runtime, || NOW, context)?)
}

/// Change the authenticated inventory identity while preserving a complete executable fixture.
fn change_package(fixture: &mut Fixture) -> Result<(), Failure> {
    let (manifest, compressed) = test_package::distribution()?;
    let mut manifest: serde_json::Value = serde_json::from_slice(&manifest)?;
    manifest["release_id"] = "replacement-package".into();
    fixture.archive = test_package::seal(&serde_json::to_vec(&manifest)?, &compressed);
    fixture.source = fixture.workspace.path().join("replacement-setup.exe");
    std::fs::write(&fixture.source, [b"inert installer".as_slice(), &fixture.archive].concat())?;
    Ok(())
}

#[test]
fn obsolete_staging_and_completed_releases_are_superseded_without_changing_health_or_files()
-> TestResult {
    for completed in [false, true] {
        let mut pipeline = worker::pipeline::Pipeline::new()?;
        let fixture = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
            let fixture = Fixture::new(&context)?;
            drop(complete(&fixture, 1, &context)?);
            accept_pending(fixture.installation.path(), &context)?;
            Ok(fixture)
        })?;
        let (mut fixture, before, previous_path, unknown, receipt) =
            pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
                let context = &context;
                if completed {
                    drop(complete(&fixture, 2, context)?);
                } else {
                    drop(
                        fixture
                            .import(SignedPackage::new(&fixture.archive, 2)?, context)?
                            .prepare(|| NOW, context)?,
                    );
                }
                let store = SelectionStore::open(fixture.installation.path(), context)?;
                let before = store.load(context)?.ok_or("Selection absent")?;
                let previous = before.candidate().ok_or("Candidate absent")?.generation();
                let previous_path =
                    fixture.installation.path().join("releases").join(previous.id.as_str());
                let unknown = previous_path.join("user-note.txt");
                std::fs::write(&unknown, b"preserve previous generation")?;
                let receipt = if completed {
                    Some(std::fs::read(previous_path.join("application-update-v1.record"))?)
                } else {
                    None
                };
                Ok((fixture, before, previous_path, unknown, receipt))
            })?;
        pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
            let context = &context;
            let previous = before.candidate().ok_or("Candidate absent")?.generation();
            let store = SelectionStore::open(fixture.installation.path(), context)?;
            if !completed {
                change_package(&mut fixture)?;
            }
            let prepared = fixture
                .import(SignedPackage::new(&fixture.archive, 3)?, context)?
                .prepare(|| NOW, context)?;
            let reserved = store.load(context)?.ok_or("Reservation absent")?;
            assert_eq!(reserved.revision(), before.revision() + 1);
            assert_eq!(reserved.accepted(), before.accepted());
            assert_eq!(reserved.retained(), before.retained());
            assert_eq!(reserved.strikes(), before.strikes());
            let replacement = reserved.candidate().ok_or("Replacement absent")?.generation();
            assert_ne!(replacement.id, previous.id);
            let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
            let candidate = prepared.complete(&runtime, || NOW, context)?;
            let directory = candidate.directory().to_owned();
            drop(candidate);
            let after = store.load(context)?;
            assert!(fixture.import(SignedPackage::new(&fixture.archive, 2)?, context).is_err());
            assert_eq!(store.load(context)?, after);
            let retry = complete(&fixture, 3, context)?;
            assert_eq!(retry.directory(), directory);
            assert_eq!(std::fs::read(unknown)?, b"preserve previous generation");
            if let Some(receipt) = receipt {
                assert_eq!(
                    std::fs::read(previous_path.join("application-update-v1.record"))?,
                    receipt
                );
            }
            fixture.preserved()
        })?;
    }
    Ok(())
}

#[test]
fn a_live_candidate_or_trial_cannot_be_superseded() -> TestResult {
    let mut pipeline = worker::pipeline::Pipeline::new()?;
    let mut fixture = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
        let fixture = Fixture::new(&context)?;
        drop(complete(&fixture, 1, &context)?);
        accept_pending(fixture.installation.path(), &context)?;
        let candidate = complete(&fixture, 2, &context)?;
        let store = SelectionStore::open(fixture.installation.path(), &context)?;
        let prepared = store.load(&context)?;
        assert!(fixture.import(SignedPackage::new(&fixture.archive, 3)?, &context).is_err());
        assert_eq!(store.load(&context)?, prepared);
        drop(candidate);
        Ok(fixture)
    })?;
    let root = fixture.installation.path().to_owned();
    let (run, live) = pipeline.run(RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(&root, &context)?);
        let mut run = InstallationRun::acquire(store.clone(), &context)?;
        run.begin_trial(&store.load(&context)?.ok_or("Selection absent")?, &context)?;
        let live = store.load(&context)?;
        Ok((run, live))
    })?;
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        change_package(&mut fixture)?;
        let imported = fixture.import(SignedPackage::new(&fixture.archive, 3)?, &context)?;
        assert!(imported.prepare(|| NOW, &context).is_err());
        let store = SelectionStore::open(fixture.installation.path(), &context)?;
        assert_eq!(store.load(&context)?, live);
        drop(run);
        fixture.preserved()
    })
}

#[test]
fn unauthenticated_replacement_preserves_pending_selection_and_payloads() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let candidate = complete(&fixture, 1, context)?;
        let executable = candidate.directory().join("sugarsubstitute.exe");
        drop(candidate);
        let bytes = std::fs::read(&executable)?;
        let store = SelectionStore::open(fixture.installation.path(), context)?;
        let before = store.load(context)?;
        let mut changed = SignedPackage::new(&fixture.archive, 2)?;
        changed.descriptor[0] = b'!';
        assert!(fixture.import(changed, context).is_err());
        assert_eq!(store.load(context)?, before);
        assert_eq!(std::fs::read(executable)?, bytes);
        fixture.preserved()
    })
}

#[test]
fn switching_channel_with_identical_payload_uses_a_fresh_publication_generation() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let old = complete(&fixture, 1, context)?;
        let previous = old.directory().to_owned();
        drop(old);
        {
            let trust = TrustStore::open(fixture.installation.path(), [92; 32], context)?;
            trust.select_subscription(trust.subscription(context)?, Channel::Canary, context)?;
        }
        let mut signed = SignedPackage::new(&fixture.archive, 2)?;
        signed.request.channel = Channel::Canary;
        let mut descriptor: serde_json::Value = serde_json::from_slice(&signed.descriptor)?;
        descriptor["channel"] = "canary".into();
        signed.descriptor = serde_json::to_vec(&descriptor)?;
        signed.metadata = Set::targets(
            &key(92)?,
            [2; 3],
            &[
                (&signed.request.target_name(), &signed.descriptor),
                (
                    &format!("{:x}.ssupdate", sha2::Sha256::digest(&fixture.archive)),
                    &fixture.archive,
                ),
            ],
            EXPIRES,
        )?;
        let prepared = fixture.import(signed, context)?.prepare(|| NOW, context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
        let replacement = prepared.complete(&runtime, || NOW, context)?;
        assert_ne!(replacement.directory(), previous);
        assert!(previous.join("sugarsubstitute.exe").is_file());
        fixture.preserved()
    })
}
