// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the composed offline path with real signatures, archive extraction and native custody.
use super::*;
use crate::setup_storage::{
    test_package::{self, maintenance::Fixture, repository::SignedPackage},
    test_worker::{self as worker, TestResult},
};
use sugarsubstitute_update::fixture::{EXPIRES, NOW, Set, key, root};
type Failure = Box<dyn std::error::Error + Send + Sync>;
mod creation_retry;
mod fresh;
mod launch_recovery;
mod online;
mod phases;
mod retry;
mod supersession;
mod transfer_recovery;

#[test]
fn composed_maintenance_preserves_configuration_and_transfers_only_an_unaccepted_trial()
-> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let imported = fixture.import(SignedPackage::new(&fixture.archive, 1)?, context)?;
        let (worker, package) = imported.worker_paths();
        assert!(worker.is_file());
        assert!(package.is_dir());
        assert!(InstallationSession::maintain(fixture.installation.path(), context).is_err());
        let prepared = imported.prepare(|| NOW, context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
        prepared.stage(context)?;
        let candidate = prepared.complete(&runtime, || NOW, context)?;
        assert_eq!(
            std::fs::read(candidate.directory().join("startup.local.toml"))?,
            std::fs::read(fixture.original.join("startup.local.toml"))?
        );
        let store = Arc::new(SelectionStore::open(fixture.installation.path(), context)?);
        assert!(store.load(context)?.ok_or("No state")?.accepted().is_none());
        let transfer = candidate.transfer(|| NOW, context, &mut None)?;
        assert!(transfer.executable().is_file());
        assert!(matches!(
            store.load(context)?.ok_or("No state")?.candidate(),
            Some(Candidate::Trial { .. })
        ));
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        assert!(std::fs::write(transfer.executable(), b"replace").is_err());
        fixture.preserved()?;
        drop(transfer);
        assert!(store.load(context)?.ok_or("No state")?.accepted().is_none());
        Ok(())
    })
}

#[test]
fn busy_launch_preserves_the_candidate_for_retry_after_application_exit() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let imported = fixture.import(SignedPackage::new(&fixture.archive, 1)?, context)?;
        let store = Arc::new(SelectionStore::open(fixture.installation.path(), context)?);
        let running = InstallationRun::acquire(store.clone(), context)?;
        let prepared = imported.prepare(|| NOW, context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
        let candidate = prepared.complete(&runtime, || NOW, context)?;
        let snapshot = store.load(context)?;
        assert!(candidate.transfer(|| NOW, context, &mut None).is_err());
        assert_eq!(store.load(context)?, snapshot);
        assert!(matches!(
            snapshot.ok_or("No state")?.candidate(),
            Some(Candidate::Prepared { .. })
        ));
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        fixture.preserved()?;
        drop(running);
        let transfer = candidate.transfer(|| NOW, context, &mut None)?;
        assert_eq!(transfer.directory(), candidate.directory());
        assert!(transfer.executable().is_file());
        let trial = store.load(context)?;
        assert!(matches!(
            trial.as_ref().and_then(|state| state.candidate()),
            Some(Candidate::Trial { .. })
        ));
        assert!(candidate.transfer(|| NOW, context, &mut None).is_err());
        assert_eq!(store.load(context)?, trial);
        drop(transfer);
        assert!(candidate.transfer(|| NOW, context, &mut None).is_err());
        assert_eq!(store.load(context)?, trial);
        fixture.preserved()?;
        Ok(())
    })
}

#[test]
fn retry_reopens_the_recorded_candidate_instead_of_allocating_another_generation() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let prepared = fixture
            .import(SignedPackage::new(&fixture.archive, 1)?, context)?
            .prepare(|| NOW, context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
        let first = prepared.complete(&runtime, || NOW, context)?;
        let path = first.directory().to_owned();
        let state = SelectionStore::open(fixture.installation.path(), context)?.load(context)?;
        drop(first);
        let retried = fixture
            .import(SignedPackage::new(&fixture.archive, 1)?, context)?
            .prepare(|| NOW, context)?;
        let candidate = retried.complete(&runtime, || NOW, context)?;
        assert_eq!(candidate.directory(), path);
        assert_eq!(
            SelectionStore::open(fixture.installation.path(), context)?.load(context)?,
            state
        );
        assert_eq!(std::fs::read_dir(fixture.installation.path().join("releases"))?.count(), 2);
        fixture.preserved()
    })
}

#[test]
fn invalid_archive_and_wrong_route_never_reserve_a_candidate() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        for bad_archive in [false, true] {
            let fixture = Fixture::new(context)?;
            let mut signed = SignedPackage::new(&fixture.archive, 1)?;
            if bad_archive {
                let mut bytes = std::fs::read(&fixture.source)?;
                bytes[b"inert installer prefix".len()] ^= 1;
                std::fs::write(&fixture.source, bytes)?;
            } else {
                signed.request.channel = sugarsubstitute_contracts::update::Channel::Canary;
            }
            assert!(fixture.import(signed, context).is_err());
            let state = SelectionStore::open(fixture.installation.path(), context)?
                .load(context)?
                .ok_or("No state")?;
            assert!(state.candidate().is_none());
            assert_eq!(std::fs::read_dir(fixture.installation.path().join("releases"))?.count(), 1);
            fixture.preserved()?;
        }
        Ok(())
    })
}

#[test]
fn foreign_engine_request_is_rejected_before_initializing_update_state() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let mut signed = SignedPackage::new(&fixture.archive, 1)?;
        signed.request.component = sugarsubstitute_contracts::update::Component::Updater;
        assert!(fixture.import(signed, context).is_err());
        assert!(!fixture.installation.path().join(".selection-state").exists());
        assert!(!fixture.installation.path().join(".update-trust").exists());
        fixture.preserved()
    })
}

#[test]
fn expiry_and_signing_rotation_after_import_prevent_candidate_publication() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        for rotate in [false, true] {
            let fixture = Fixture::new(context)?;
            let imported = fixture.import(SignedPackage::new(&fixture.archive, 1)?, context)?;
            let prepared = imported.prepare(|| NOW, context)?;
            let before =
                SelectionStore::open(fixture.installation.path(), context)?.load(context)?;
            if rotate {
                let trust = TrustStore::open(fixture.installation.path(), [92; 32], context)?;
                trust.rotate(&root(&key(93)?, 2, Some(&key(92)?))?, context)?;
            }
            let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
            assert!(
                prepared
                    .complete(&runtime, || if rotate { NOW } else { EXPIRES }, context)
                    .is_err()
            );
            assert_eq!(
                SelectionStore::open(fixture.installation.path(), context)?.load(context)?,
                before
            );
            fixture.preserved()?;
        }
        Ok(())
    })
}

#[test]
fn offline_root_chain_advances_existing_trust_without_resetting_its_identity() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        drop(fixture.import(SignedPackage::new(&fixture.archive, 1)?, context)?);
        let before = SelectionStore::open(fixture.installation.path(), context)?.load(context)?;
        let mut signed = SignedPackage::new(&fixture.archive, 2)?;
        let old = key(92)?;
        let next = key(93)?;
        let next_root = root(&next, 2, Some(&old))?;
        let digest = format!("{:x}", sha2::Sha256::digest(&fixture.archive));
        signed.metadata = Set::targets(
            &next,
            [2; 3],
            &[
                (&signed.request.target_name(), &signed.descriptor),
                (&format!("{digest}.ssupdate"), &fixture.archive),
            ],
            EXPIRES,
        )?;
        let imported = InstallationSession::maintain(fixture.installation.path(), context)?
            .import_installer(
                InstallerSource {
                    repository: [92; 32],
                    trusted_root: &next_root,
                    roots: &[(2, &next_root)],
                    metadata: signed.metadata.metadata(),
                    descriptor: &signed.descriptor,
                    executable: &fixture.source,
                },
                signed.request,
                fixture.workspace.path(),
                || NOW,
                context,
            )?;
        assert_eq!(imported.trust.next_root_version(context)?, 3);
        assert_eq!(
            SelectionStore::open(fixture.installation.path(), context)?.load(context)?,
            before
        );
        fixture.preserved()
    })
}

use sha2::Digest;
use sugarsubstitute_contracts::installation::Candidate;
