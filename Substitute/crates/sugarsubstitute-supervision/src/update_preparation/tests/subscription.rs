// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Local preferences govern actual discovery and invalidate stale automatic trial authority.
use super::*;
use sugarsubstitute_contracts::update::Channel;
use sugarsubstitute_update::fixture::{EXPIRES, Set, key};

#[test]
fn saved_canary_preference_drives_real_signed_discovery_and_preparation() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, mut signed) = initialized(&mut pipeline)?;
    select_subscription(&fixture, Channel::Canary)?;
    signed.request.channel = Channel::Canary;
    let mut descriptor: serde_json::Value = serde_json::from_slice(&signed.descriptor)?;
    descriptor["channel"] = "canary".into();
    signed.descriptor = serde_json::to_vec(&descriptor)?;
    signed.metadata = Set::targets(
        &key(92)?,
        [1; 3],
        &[
            (&signed.request.target_name(), &signed.descriptor),
            (&format!("{:x}.ssupdate", Sha256::digest(&fixture.archive)), &fixture.archive),
        ],
        EXPIRES,
    )?;
    let (mut worker, server) = campaign(&mut pipeline, &fixture, signed, true)?;
    for _ in 0..5 {
        step(&mut pipeline, &mut worker)?;
    }
    assert_eq!(worker.progress(), Progress::Ready);
    server.finish()?;
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    fixture.preserved()
}

#[test]
fn preference_changed_after_open_stops_before_network_even_after_channel_round_trip() -> TestResult
{
    for round_trip in [false, true] {
        let mut pipeline = Pipeline::new()?;
        let (fixture, _) = initialized(&mut pipeline)?;
        let mut worker = Worker::new(configuration(
            &fixture,
            sugarsubstitute_update::http::Policy::loopback(1)?,
        ))?;
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::Checking);
        select_subscription(&fixture, Channel::Canary)?;
        if round_trip {
            select_subscription(&fixture, Channel::Release)?;
        }
        submit(&mut pipeline, &mut worker)?;
        let observed = pipeline.observe(|| worker.poll())?;
        assert_eq!(observed.progress, Progress::Failed);
        assert!(format!("{:?}", observed.failure).contains("subscription changed"));
        retire(&mut pipeline, &mut worker)?;
        assert_unlocked(&mut pipeline, &fixture)?;
    }
    Ok(())
}

#[test]
fn changed_preference_stops_authenticated_download_before_requesting_package() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut pipeline)?;
    let (mut worker, server) = campaign(&mut pipeline, &fixture, signed, false)?;
    step(&mut pipeline, &mut worker)?;
    assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::Reopening);
    assert!(matches!(step(&mut pipeline, &mut worker)?.progress, Progress::Downloading { .. }));
    server.finish()?;
    select_subscription(&fixture, Channel::Canary)?;
    submit(&mut pipeline, &mut worker)?;
    assert_eq!(pipeline.observe(|| worker.poll())?.progress, Progress::Failed);
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)
}

#[test]
fn prepared_candidate_cannot_activate_after_subscription_change() -> TestResult {
    let (fixture, _) = restart::prepared()?;
    let mut pipeline = Pipeline::new()?;
    let mut worker = Worker::resume_cached(cached_configuration(&fixture))?;
    for _ in 0..3 {
        step(&mut pipeline, &mut worker)?;
    }
    let candidate = worker.take_candidate()?;
    select_subscription(&fixture, Channel::Canary)?;
    let installation = fixture.installation.path().to_owned();
    pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(&installation, &context)?);
        let before = store.load(&context)?;
        assert!(candidate.transfer(|| NOW, &context, &mut None).is_err());
        assert_eq!(store.load(&context)?, before);
        drop(candidate);
        assert!(InstallationRun::acquire(store, &context).is_ok());
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    fixture.preserved()
}

#[test]
fn explicit_offline_installer_repair_preserves_the_saved_subscription() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, mut signed) = initialized(&mut pipeline)?;
    select_subscription(&fixture, Channel::Canary)?;
    signed.request.intent = sugarsubstitute_contracts::installation::PreparationIntent::Repair;
    let fixture = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        let prepared = fixture.import(signed, &context)?.prepare(|| NOW, &context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Runtime missing")?.clone();
        drop(prepared.complete(&runtime, || NOW, &context)?);
        Ok(fixture)
    })?;
    let installation = fixture.installation.path().to_owned();
    pipeline.run(TRUST_RESERVATION_BYTES, move |context| {
        let trust = TrustStore::open(&installation, [92; 32], &context)?;
        assert_eq!(trust.subscription(&context)?.channel(), Channel::Canary);
        Ok(())
    })?;
    fixture.preserved()
}
