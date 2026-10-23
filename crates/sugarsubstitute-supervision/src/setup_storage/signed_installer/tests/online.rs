// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Join real HTTP delivery and offline delivery to identical retained preparation custody.
use super::*;
use crate::{
    update_http::fixture::{Running, complete, exchange},
    update_repository::{Check, Preparation, Repository},
};
use sha2::{Digest, Sha256};
use std::time::Duration;
use sugarsubstitute_update::{Error as Verification, RepositoryTrust};

type Delivery = (TrustStore, AuthenticatedOffer, VerifiedDownload);

#[test]
fn download_and_import_jobs_share_retained_capacity_without_reserving_a_second_peak() -> TestResult
{
    use crate::setup_storage::test_worker::pipeline::Pipeline;
    let mut pipeline = Pipeline::new()?;
    let (fixture, trust, offer, download) =
        pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
            let fixture = Fixture::new(&context)?;
            let (trust, offer, download) = delivered(&fixture, &context)?;
            Ok((fixture, trust, offer, download))
        })?;
    let rejected = pipeline
        .run_once(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))
        .err()
        .ok_or("Second peak reservation was admitted")?;
    assert!(matches!(
        rejected.downcast_ref::<sugarsubstitute_contracts::execution::AdmissionError>(),
        Some(sugarsubstitute_contracts::execution::AdmissionError::MemorySaturated { .. })
    ));
    let installation = fixture.installation.path().to_owned();
    let workspace = fixture.workspace.path().to_owned();
    let imported = pipeline.run(SIGNED_PREPARATION_RESERVATION_BYTES, move |context| {
        Ok(InstallationSession::maintain(&installation, &context)?.import_download(
            trust,
            offer,
            download,
            &workspace,
            || NOW,
            &context,
        )?)
    })?;
    let prepared = pipeline.run(SIGNED_PREPARATION_RESERVATION_BYTES, move |context| {
        Ok(imported.prepare(|| NOW, &context)?)
    })?;
    let candidate = pipeline.run(SIGNED_PREPARATION_RESERVATION_BYTES, move |context| {
        let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
        prepared.stage(&context)?;
        Ok(prepared.complete(&runtime, || NOW, &context)?)
    })?;
    fixture.preserved()?;
    drop(candidate);
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))?;
    Ok(())
}

/// Establish fixture authority independently of the delivery being tested.
fn trust(
    fixture: &Fixture,
    signed: &SignedPackage,
    context: &JobContext,
) -> Result<TrustStore, Failure> {
    Ok(InstallationSession::maintain(fixture.installation.path(), context)?.update_trust(
        [92; 32],
        &signed.root,
        context,
    )?)
}

/// Supply authenticated retained bytes without involving HTTP in rejection tests.
fn delivered(fixture: &Fixture, context: &JobContext) -> Result<Delivery, Failure> {
    let signed = SignedPackage::new(&fixture.archive, 1)?;
    let trust = trust(fixture, &signed, context)?;
    let offer = AuthenticatedOffer::authenticate(
        &trust,
        signed.request,
        signed.metadata.metadata(),
        &signed.descriptor,
        NOW,
        context,
    )?;
    let authorization = offer.authorization(&trust);
    let target = trust
        .offer(
            authorization.metadata,
            authorization.request,
            authorization.descriptor,
            NOW,
            context,
        )?
        .into_package()?;
    let source = fixture.workspace.path().join("download.ssupdate");
    std::fs::write(&source, &fixture.archive)?;
    let download =
        Download::import_file(fixture.installation.path(), target, &source, || NOW, context)?;
    Ok((trust, offer, download))
}

#[test]
fn http_delivery_enters_shared_preparation_while_application_run_exclusion_remains_held()
-> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let signed = SignedPackage::new(&fixture.archive, 1)?;
        let trust = trust(&fixture, &signed, context)?;
        let store = Arc::new(SelectionStore::open(fixture.installation.path(), context)?);
        let run = InstallationRun::acquire(store.clone(), context)?;
        let before = store.load(context)?;
        let package_name = format!("{:x}.ssupdate", Sha256::digest(&fixture.archive));
        let exchanges = vec![
            exchange("2.root.json", None, 404, "Content-Length: 0\r\n", b""),
            complete("timestamp.json", &signed.metadata.timestamp),
            complete("1.snapshot.json", &signed.metadata.snapshot),
            complete("1.targets.json", &signed.metadata.targets),
            complete(&signed.request.target_name(), &signed.descriptor),
            exchange(
                &package_name,
                Some("bytes=0-"),
                200,
                &format!("Content-Length: {}\r\n", fixture.archive.len()),
                &fixture.archive,
            ),
        ];
        let (server, policy) = Running::start(exchanges, context)?;
        let mut repository = Repository::new(policy, Duration::from_secs(1));
        let Check::Offer(offer) = repository.discover(&trust, signed.request, || NOW, context)?
        else {
            return Err("Expected complete discovery".into());
        };
        let Preparation::Complete(download) =
            repository.prepare(&offer, &trust, fixture.installation.path(), || NOW, context)?
        else {
            return Err("Expected complete download".into());
        };
        assert!(repository.settled());
        server.finish()?;
        let imported = InstallationSession::maintain(fixture.installation.path(), context)?
            .import_download(trust, *offer, *download, fixture.workspace.path(), || NOW, context)?;
        assert_eq!(store.load(context)?, before);
        let prepared = imported.prepare(|| NOW, context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
        prepared.stage(context)?;
        let candidate = prepared.complete(&runtime, || NOW, context)?;
        let after = store.load(context)?.ok_or("Selection absent")?;
        assert_eq!(after.accepted(), before.as_ref().and_then(|record| record.accepted()));
        assert!(matches!(after.candidate(), Some(Candidate::Prepared { .. })));
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        assert!(candidate.transfer(|| NOW, context, &mut None).is_err());
        assert_eq!(store.load(context)?, Some(after));
        fixture.preserved()?;
        drop(run);
        let _next_run = InstallationRun::acquire(store, context)?;
        Ok(())
    })
}

#[test]
fn foreign_installation_trust_is_rejected_before_extraction_or_selection_changes() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let source = Fixture::new(context)?;
        let (trust, offer, download) = delivered(&source, context)?;
        let destination = Fixture::new(context)?;
        let extraction = tempfile::tempdir()?;
        assert!(
            InstallationSession::maintain(destination.installation.path(), context)?
                .import_download(trust, offer, download, extraction.path(), || NOW, context,)
                .is_err()
        );
        assert_eq!(std::fs::read_dir(extraction.path())?.count(), 0);
        assert!(SelectionStore::existing(destination.installation.path(), context)?.is_none());
        source.preserved()?;
        destination.preserved()
    })
}

#[test]
fn verified_bytes_for_another_signed_package_cannot_enter_the_imported_offer() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let (trust, offer, original) = delivered(&fixture, context)?;
        drop(original);
        let other = b"different authenticated archive";
        let signed = SignedPackage::new(other, 1)?;
        let mut verifier = RepositoryTrust::bootstrap(&signed.root)?;
        verifier.refresh(signed.metadata.metadata(), NOW)?;
        let target = verifier.offer(&signed.request, &signed.descriptor, NOW)?.into_package()?;
        let source = fixture.workspace.path().join("other.ssupdate");
        std::fs::write(&source, other)?;
        let download =
            Download::import_file(fixture.installation.path(), target, &source, || NOW, context)?;
        let extraction = tempfile::tempdir()?;
        let result = InstallationSession::maintain(fixture.installation.path(), context)?
            .import_download(trust, offer, download, extraction.path(), || NOW, context);
        assert!(matches!(result, Err(TrustFailure::Verification(Verification::Digest))));
        assert_eq!(std::fs::read_dir(extraction.path())?.count(), 0);
        fixture.preserved()
    })
}

#[test]
fn expired_delivery_is_rejected_before_archive_extraction() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let (trust, offer, download) = delivered(&fixture, context)?;
        let extraction = tempfile::tempdir()?;
        let result = InstallationSession::maintain(fixture.installation.path(), context)?
            .import_download(trust, offer, download, extraction.path(), || EXPIRES, context);
        assert!(matches!(result, Err(TrustFailure::Verification(Verification::Expired))));
        assert_eq!(std::fs::read_dir(extraction.path())?.count(), 0);
        fixture.preserved()
    })
}

#[test]
fn extraction_cannot_outlive_authority_or_roll_the_clock_back() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        for after in [EXPIRES, NOW - 1] {
            let fixture = Fixture::new(context)?;
            let (trust, offer, download) = delivered(&fixture, context)?;
            let store = SelectionStore::open(fixture.installation.path(), context)?;
            let before = store.load(context)?;
            let extraction = tempfile::tempdir()?;
            let mut samples = 0;
            let result = InstallationSession::maintain(fixture.installation.path(), context)?
                .import_download(
                    trust,
                    offer,
                    download,
                    extraction.path(),
                    || {
                        samples += 1;
                        if samples == 1 { NOW } else { after }
                    },
                    context,
                );
            assert_eq!(samples, 2);
            assert!(matches!(
                result,
                Err(TrustFailure::Verification(
                    Verification::Expired | Verification::ClockRollback
                ))
            ));
            assert_eq!(store.load(context)?, before);
            assert!(std::fs::read_dir(extraction.path())?.count() > 0);
            fixture.preserved()?;
        }
        Ok(())
    })
}
