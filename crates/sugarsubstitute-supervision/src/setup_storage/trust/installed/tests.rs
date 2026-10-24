// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Healthy request identity follows native acceptance, never downloaded or failed candidates.
use super::*;
use crate::setup_storage::{
    InstallationRun, SIGNED_INSTALLER_RESERVATION_BYTES, SignedCandidate,
    test_package::{maintenance::Fixture, repository::SignedPackage},
    test_worker::{self as worker, TestResult},
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use sugarsubstitute_contracts::update::Admission;
use sugarsubstitute_update::fixture::NOW;

/// Exercise the shared signed installer path before any fixture publishes health.
fn prepare(
    fixture: &Fixture,
    version: u32,
    context: &JobContext,
) -> Result<SignedCandidate, Box<dyn std::error::Error + Send + Sync>> {
    let prepared = fixture
        .import(SignedPackage::new(&fixture.archive, version)?, context)?
        .prepare(|| NOW, context)?;
    let runtime = prepared.maintenance_runtime().ok_or("Runtime missing")?.clone();
    Ok(prepared.complete(&runtime, || NOW, context)?)
}

/// Native run ownership supplies a test health decision; reading release facts cannot create it.
fn accept(store: &Arc<SelectionStore>, context: &JobContext) -> TestResult {
    let mut run = InstallationRun::acquire(store.clone(), context)?;
    let snapshot = store.load(context)?.ok_or("Selection missing")?;
    let trial = run.begin_trial(&snapshot, context)?;
    run.accept(&trial, context)?;
    Ok(())
}

#[test]
fn healthy_identity_advances_only_after_acceptance_and_survives_a_failed_newer_release()
-> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let first = prepare(&fixture, 1, context)?;
        let store = Arc::new(SelectionStore::open(fixture.installation.path(), context)?);
        let trust = TrustStore::open(fixture.installation.path(), [92; 32], context)?;
        assert_eq!(trust.application_request(Channel::Release, context)?.sequence_floor, 0);
        {
            let mut run = InstallationRun::acquire(store.clone(), context)?;
            let trial =
                run.begin_trial(&store.load(context)?.ok_or("Selection missing")?, context)?;
            assert_eq!(trust.application_request(Channel::Release, context)?.sequence_floor, 0);
            run.accept(&trial, context)?;
        }
        let accepted = trust.application_request(Channel::Release, context)?;
        assert_eq!(accepted.sequence_floor, 1);
        assert_eq!(accepted.sequence_digest, Some(Sha256::digest(&fixture.archive).into()));
        assert_eq!(accepted.updater_release, sugarsubstitute_update::ENGINE_RELEASE);
        assert_eq!(accepted.updater_protocol, sugarsubstitute_update::ENGINE_PROTOCOL);
        assert_eq!(accepted.package_schema, sugarsubstitute_update::package::SCHEMA_VERSION);
        assert_eq!(trust.application_request(Channel::Canary, context)?.sequence_floor, 0);
        drop(first);
        let newer = prepare(&fixture, 2, context)?;
        assert_eq!(trust.application_request(Channel::Release, context)?.sequence_floor, 1);
        {
            let mut run = InstallationRun::acquire(store.clone(), context)?;
            let trial =
                run.begin_trial(&store.load(context)?.ok_or("Selection missing")?, context)?;
            run.reject(&trial, context)?;
        }
        drop(newer);
        let request = trust.application_request(Channel::Release, context)?;
        assert_eq!(request.sequence_floor, 1);
        let signed = SignedPackage::new(&fixture.archive, 2)?;
        assert_eq!(
            trust
                .offer(signed.metadata.metadata(), &request, &signed.descriptor, NOW, context)?
                .admission(),
            Admission::Prepare
        );
        let older = SignedPackage::new(&fixture.archive, 1)?;
        assert!(
            trust
                .offer(older.metadata.metadata(), &request, &older.descriptor, NOW, context)
                .is_err()
        );
        assert_eq!(store.load(context)?.ok_or("Selection missing")?.rejected().len(), 1);
        fixture.preserved()
    })
}

#[test]
fn receipt_reopen_rejects_corruption_foreign_binding_and_unknown_schemas_without_repair()
-> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let candidate = prepare(&fixture, 1, context)?;
        let store = Arc::new(SelectionStore::open(fixture.installation.path(), context)?);
        accept(&store, context)?;
        let path = candidate.directory().join(RECORD);
        drop(candidate);
        let trust = TrustStore::open(fixture.installation.path(), [92; 32], context)?;
        let original = std::fs::read(&path)?;
        let payload = envelope::decode(
            envelope::Kind::InstalledRelease,
            &trust.installation,
            &trust.repository,
            &original,
        )?;
        for case in [
            "schema",
            "generation",
            "component",
            "platform",
            "zero",
            "repository",
            "installation",
            "checksum",
        ] {
            let mut value: serde_json::Value = serde_json::from_slice(payload)?;
            match case {
                "schema" => value["schema_version"] = 2.into(),
                "generation" => value["generation"]["id"] = "a".repeat(32).into(),
                "component" => value["component"] = "updater".into(),
                "platform" => value["platform"] = "macos-x86_64".into(),
                "zero" => value["sequence"] = 0.into(),
                _ => {}
            }
            let mut bytes = envelope::encode(
                envelope::Kind::InstalledRelease,
                &if case == "installation" {
                    InstallationId::new("f".repeat(32))?
                } else {
                    trust.installation.clone()
                },
                &if case == "repository" { [93; 32] } else { trust.repository },
                &serde_json::to_vec(&value)?,
            )?;
            if case == "checksum" {
                bytes[100] ^= 1;
            }
            std::fs::write(&path, &bytes)?;
            assert!(
                trust.application_request(Channel::Release, context).is_err(),
                "accepted {case}"
            );
            assert_eq!(std::fs::read(&path)?, bytes);
        }
        std::fs::write(&path, &original)?;
        assert_eq!(trust.application_request(Channel::Release, context)?.sequence_floor, 1);
        let ordinary = fixture.installation.path().join("unprotected-receipt");
        std::fs::write(&ordinary, &original)?;
        std::fs::remove_file(&path)?;
        std::fs::rename(&ordinary, &path)?;
        assert!(trust.application_request(Channel::Release, context).is_err());
        assert_eq!(std::fs::read(&path)?, original);
        Ok(())
    })
}

#[test]
fn older_native_generation_without_receipt_keeps_trust_and_has_unknown_healthy_identity()
-> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let candidate = prepare(&fixture, 1, context)?;
        let store = Arc::new(SelectionStore::open(fixture.installation.path(), context)?);
        accept(&store, context)?;
        let path = candidate.directory().join(RECORD);
        drop(candidate);
        std::fs::remove_file(path)?;
        let trust = TrustStore::open(fixture.installation.path(), [92; 32], context)?;
        let checkpoint = std::fs::read(trust.root.path.join(super::super::RECORD))?;
        let request = trust.application_request(Channel::Release, context)?;
        assert_eq!(request.sequence_floor, 0);
        assert_eq!(request.sequence_digest, None);
        assert_eq!(std::fs::read(trust.root.path.join(super::super::RECORD))?, checkpoint);
        assert!(store.load(context)?.ok_or("Selection missing")?.accepted().is_some());
        fixture.preserved()
    })
}

#[test]
fn completed_receipt_is_immutable_and_conflicting_retry_preserves_selection_and_bytes() -> TestResult
{
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let candidate = prepare(&fixture, 1, context)?;
        let path = candidate.directory().join(RECORD);
        let original = std::fs::read(&path)?;
        let store = SelectionStore::open(fixture.installation.path(), context)?;
        let before = store.load(context)?;
        drop(candidate);
        let retry = prepare(&fixture, 1, context)?;
        assert_eq!(retry.directory().join(RECORD), path);
        assert_eq!(std::fs::read(&path)?, original);
        drop(retry);
        std::fs::write(&path, b"unknown")?;
        assert!(prepare(&fixture, 1, context).is_err());
        assert_eq!(std::fs::read(&path)?, b"unknown");
        assert_eq!(store.load(context)?, before);
        fixture.preserved()
    })
}
