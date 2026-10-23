// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real replacement failures must withhold authority without losing original trust bytes.
use super::*;
use sugarsubstitute_contracts::update::{Admission, Channel, Component, Platform};

/// One complete signed offer and its locally owned admission facts.
struct OfferFixture {
    root: Vec<u8>,
    metadata: Set,
    descriptor: Vec<u8>,
    request: Request,
}

/// Sign a genuine release descriptor and its content-addressed package in one repository.
fn offer_fixture() -> Result<OfferFixture, Box<dyn std::error::Error + Send + Sync>> {
    let key = key(20)?;
    let package = b"authenticated native fixture package";
    let digest = format!("{:x}", Sha256::digest(package));
    let descriptor = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1, "product": "sugarsubstitute", "channel": "release",
        "platform": "windows-x86_64", "component": "application", "sequence": 1,
        "version": "1.0.0", "package_sha256": digest, "package_bytes": package.len(),
        "package_schema": 1, "updater_protocol": 1, "minimum_updater_release": 1,
    }))?;
    let request = Request {
        channel: Channel::Release,
        platform: Platform::WindowsX86_64,
        component: Component::Application,
        sequence_floor: 0,
        sequence_digest: None,
        updater_release: 1,
        updater_protocol: 1,
        package_schema: 1,
        intent: PreparationIntent::Automatic,
    };
    let set = Set::targets(
        &key,
        [1; 3],
        &[(&request.target_name(), &descriptor), (&format!("{digest}.ssupdate"), package)],
        EXPIRES,
    )?;
    Ok(OfferFixture { root: root(&key, 1, None)?, metadata: set, descriptor, request })
}

#[test]
fn an_offer_is_returned_only_after_real_durable_publication() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(20, context)?;
        let OfferFixture { root, metadata, descriptor, request } = offer_fixture()?;
        let store = TrustStore::initialize(
            installation.path(),
            Sha256::digest(&root).into(),
            &root,
            context,
        )?;
        let original = std::fs::read(store.root.path.join(RECORD))?;
        let pin = store.root.read(RECORD)?;
        assert!(matches!(
            store.offer(metadata.metadata(), &request, &descriptor, NOW, context),
            Err(TrustFailure::Publication { verification: None, .. })
        ));
        assert_eq!(std::fs::read(store.root.path.join(RECORD))?, original);
        drop(pin);
        let offer = store.offer(metadata.metadata(), &request, &descriptor, NOW, context)?;
        assert_eq!(offer.admission(), Admission::Prepare);
        assert_ne!(std::fs::read(store.root.path.join(RECORD))?, original);
        let reopened = TrustStore::open(installation.path(), Sha256::digest(root).into(), context)?;
        assert!(matches!(
            reopened.refresh(
                Set::new(&key(20)?, [1; 3], b"different", EXPIRES)?.metadata(),
                NOW,
                context
            ),
            Err(TrustFailure::Verification(Verification::Equivocation))
        ));
        Ok(())
    })
}

#[test]
fn publication_failure_retains_both_causes_and_interrupted_files_are_not_authority() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(21, context)?;
        let key = key(21)?;
        let root = root(&key, 1, None)?;
        let store = TrustStore::initialize(
            installation.path(),
            Sha256::digest(&root).into(),
            &root,
            context,
        )?;
        let original = std::fs::read(store.root.path.join(RECORD))?;
        let pending = store.root.path.join("pending-interrupted");
        std::fs::write(&pending, b"partial")?;
        let metadata = Set::new(&key, [3; 3], b"current", EXPIRES)?;
        let pin = store.root.read(RECORD)?;
        assert!(matches!(
            store.refresh(
                MetadataSet {
                    timestamp: &metadata.timestamp,
                    snapshot: b"invalid",
                    targets: &metadata.targets,
                },
                NOW,
                context
            ),
            Err(TrustFailure::Publication { verification: Some(_), .. })
        ));
        assert_eq!(std::fs::read(store.root.path.join(RECORD))?, original);
        drop(pin);
        store.refresh(metadata.metadata(), NOW, context)?;
        assert_eq!(std::fs::read(&pending)?, b"partial");
        Ok(())
    })
}

#[test]
fn unprotected_directory_or_record_cannot_supply_trusted_bytes() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(22, context)?;
        let root = root(&key(22)?, 1, None)?;
        let repository = Sha256::digest(&root).into();
        let store = TrustStore::initialize(
            installation.path(),
            Sha256::digest(&root).into(),
            &root,
            context,
        )?;
        let path = store.root.path.join(RECORD);
        let bytes = std::fs::read(&path)?;
        let foreign = installation.path().join("ordinary-record");
        std::fs::write(&foreign, &bytes)?;
        std::fs::remove_file(&path)?;
        std::fs::rename(&foreign, &path)?;
        assert!(TrustStore::open(installation.path(), repository, context).is_err());
        assert_eq!(std::fs::read(&path)?, bytes);
        Ok(())
    })
}

#[test]
fn expiry_after_verification_commits_security_floors_without_returning_authority() -> TestResult {
    worker::run_reserved_expiring(TRUST_RESERVATION_BYTES, |context, clock| {
        let installation = installation(23, context)?;
        let key = key(23)?;
        let root = root(&key, 1, None)?;
        let store = TrustStore::initialize(
            installation.path(),
            Sha256::digest(&root).into(),
            &root,
            context,
        )?;
        let path = store.root.path.join(RECORD);
        let previous = std::fs::read(&path)?;
        let metadata = Set::new(&key, [4; 3], b"current", EXPIRES)?;
        let outcome = store.transition(context, |trust| {
            trust.refresh(metadata.metadata(), NOW)?;
            clock.expire();
            trust.target(NAME, NOW)
        });
        assert!(matches!(outcome, Err(TrustFailure::Storage(_))));
        let committed = std::fs::read(&path)?;
        assert_ne!(committed, previous);
        let checkpoint = envelope::decode(
            envelope::Kind::Checkpoint,
            &store.installation,
            &store.repository,
            &committed,
        )?;
        let mut recovered = RepositoryTrust::from_trusted_checkpoint(checkpoint)?;
        assert!(matches!(
            recovered.refresh(Set::new(&key, [3; 3], b"old", EXPIRES)?.metadata(), NOW,),
            Err(Verification::Rollback)
        ));
        let before = std::fs::read(&path)?;
        assert!(store.refresh(metadata.metadata(), NOW, context).is_err());
        assert_eq!(std::fs::read(&path)?, before);
        Ok(())
    })
}
