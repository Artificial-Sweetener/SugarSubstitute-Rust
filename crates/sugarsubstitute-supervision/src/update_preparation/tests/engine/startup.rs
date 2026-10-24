// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Cached engine delegation authenticates local evidence and never requests a network download.
use super::*;

fn cached(pipeline: &mut Pipeline, fixture: &Fixture) -> Result<Worker, Failure> {
    let mut signed = SignedPackage::new(&fixture.archive, 3)?;
    requires(&mut signed, &fixture.archive, "minimum_updater_release", 2)?;
    signed.metadata = sugarsubstitute_update::fixture::Set::targets(
        &sugarsubstitute_update::fixture::key(92)?,
        [3; 3],
        &[
            (&signed.request.target_name(), &signed.descriptor),
            (&format!("{:x}.ssupdate", Sha256::digest(&fixture.archive)), &fixture.archive),
        ],
        sugarsubstitute_update::fixture::EXPIRES,
    )?;
    let root = fixture.installation.path().to_owned();
    pipeline.run(
        TRUST_RESERVATION_BYTES
            + OFFER_RESERVATION_BYTES
            + OFFER_PUBLICATION_RESERVATION_BYTES
            + TRUST_OPERATION_RESERVATION_BYTES,
        move |context| {
            let trust = TrustStore::open(&root, [92; 32], &context)?;
            let offer = AuthenticatedOffer::authenticate(
                &trust,
                signed.request,
                signed.metadata.metadata(),
                &signed.descriptor,
                NOW,
                &context,
            )?;
            trust.remember_offer(&offer, &context)?;
            Ok(())
        },
    )?;
    Ok(Worker::resume_cached(cached_configuration(fixture))?)
}

#[test]
fn cached_startup_transfers_only_an_observed_accepted_activation_engine() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    seed_accepted(&mut pipeline, &fixture)?;
    let mut worker = cached(&mut pipeline, &fixture)?;
    assert!(worker.take_activation().is_err());
    for expected in [Progress::Restoring, Progress::SelectingEngine, Progress::ActivationReady] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    assert!(worker.take_candidate().is_err());
    assert!(worker.take_engine().is_err());
    assert!(submit(&mut pipeline, &mut worker).is_err());
    let image = worker.take_activation()?;
    assert_eq!(worker.progress(), Progress::Closed);
    assert!(worker.take_activation().is_err());
    pipeline.run(4096, move |_| {
        image.require_activation()?;
        assert!(std::fs::OpenOptions::new().write(true).open(image.executable()).is_err());
        drop(image);
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    fixture.preserved()
}

#[test]
fn absent_accepted_engine_is_unavailable_without_network_or_trial_publication() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    let mut worker = cached(&mut pipeline, &fixture)?;
    for expected in [Progress::Restoring, Progress::SelectingEngine, Progress::Unavailable] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    assert!(worker.take_activation().is_err());
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    fixture.preserved()
}

#[test]
fn cancelling_cached_activation_retires_file_pins_before_another_campaign() -> TestResult {
    for completed in 1..=3 {
        let mut pipeline = Pipeline::new()?;
        let (fixture, _) = initialized(&mut pipeline)?;
        seed_accepted(&mut pipeline, &fixture)?;
        let mut worker = cached(&mut pipeline, &fixture)?;
        for _ in 0..completed {
            step(&mut pipeline, &mut worker)?;
        }
        worker.cancel();
        assert!(worker.take_activation().is_err());
        retire(&mut pipeline, &mut worker)?;
        assert_unlocked(&mut pipeline, &fixture)?;
        fixture.preserved()?;
    }
    Ok(())
}
