// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Signed application requirements drive real local engine discovery, staging and owned transfer.
use super::*;
use crate::{setup_storage::test_package::updater, update_preparation::EngineUpdate};
use sugarsubstitute_contracts::{
    application_launch::updater::CURRENT_UPDATER_CONTRACT,
    installation::Candidate,
    update::{Channel, Component},
};
mod feed;
mod startup;
use feed::{Feed, campaign, seed_accepted};

const NEXT_ENGINE: u64 = sugarsubstitute_update::ENGINE_RELEASE + 1;

#[test]
fn required_engine_is_staged_through_signed_feed_without_taking_application_ownership() -> TestResult
{
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    let (mut worker, server) = campaign(&mut pipeline, &fixture, Feed::default())?;
    let root = fixture.installation.path().to_owned();
    let (store, run, before) = pipeline.run(RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(&root, &context)?);
        let run = InstallationRun::acquire(store.clone(), &context)?;
        let before = store.load(&context)?;
        Ok((store, run, before))
    })?;
    assert!(worker.take_engine().is_err());
    for expected in [Progress::Checking, Progress::SelectingEngine, Progress::CheckingEngine] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    assert!(matches!(
        step(&mut pipeline, &mut worker)?.progress,
        Progress::Downloading { received: 0, .. }
    ));
    assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::PreparingEngine);
    assert!(worker.take_engine().is_err());
    assert_eq!(step(&mut pipeline, &mut worker)?.progress, Progress::EngineReady);
    server.finish()?;
    assert!(worker.take_candidate().is_err());
    assert!(submit(&mut pipeline, &mut worker).is_err());
    let EngineUpdate::Prepared(prepared) = worker.take_engine()? else {
        return Err("Engine did not require a probe".into());
    };
    let generation = prepared.generation().clone();
    assert_eq!(worker.progress(), Progress::Closed);
    assert!(worker.take_engine().is_err());
    let root = fixture.installation.path().to_owned();
    pipeline.run(RESERVATION_BYTES, move |context| {
        drop(InstallationSession::maintain(&root, &context)?);
        assert_eq!(store.load(&context)?, before);
        assert!(InstallationRun::acquire(store, &context).is_err());
        drop((run, prepared));
        Ok(())
    })?;
    let root = fixture.installation.path().to_owned();
    pipeline.run(TRUST_RESERVATION_BYTES + TRUST_OPERATION_RESERVATION_BYTES, move |context| {
        let trust = TrustStore::open(&root, [92; 32], &context)?;
        let selection = trust.updater_selection(&context)?;
        assert_eq!(selection.candidate(), Some(&Candidate::Prepared { generation }));
        assert!(selection.accepted().is_none());
        assert_eq!(trust.updater_request(Channel::Release, &context)?.sequence_floor, 0);
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}

#[test]
fn accepted_engine_is_reused_without_fetching_an_engine_offer_or_retaining_preparation_lock()
-> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    seed_accepted(&mut pipeline, &fixture)?;
    let (mut worker, server) =
        campaign(&mut pipeline, &fixture, Feed { completed: 2, ..Feed::default() })?;
    for expected in [Progress::Checking, Progress::SelectingEngine, Progress::EngineReady] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    server.finish()?;
    let EngineUpdate::Accepted(image) = worker.take_engine()? else {
        return Err("Healthy engine was restaged".into());
    };
    let root = fixture.installation.path().to_owned();
    let workspace = fixture.workspace.path().to_owned();
    pipeline.run(TRUST_RESERVATION_BYTES + TRUST_OPERATION_RESERVATION_BYTES, move |context| {
        use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId};
        let executable = image.executable();
        let input = crate::WindowsLaunchInput::preparation(
            CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([1; 16])?,
            },
            *image,
            &workspace,
            crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
        )?;
        let mut trust = TrustStore::open(&root, [92; 32], &context)?;
        trust.acquire_preparation(&context)?;
        let request = trust.updater_request(Channel::Release, &context)?;
        assert_eq!(request.sequence_floor, NEXT_ENGINE);
        assert_eq!(request.updater_release, sugarsubstitute_update::ENGINE_RELEASE);
        assert!(std::fs::OpenOptions::new().write(true).open(executable).is_err());
        drop(input);
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}

#[test]
fn cancellation_at_each_engine_boundary_retires_custody_without_claiming_health() -> TestResult {
    for completed in 2..=6 {
        let mut pipeline = Pipeline::new()?;
        let (fixture, _) = initialized(&mut pipeline)?;
        let (mut worker, server) =
            campaign(&mut pipeline, &fixture, Feed { completed, ..Feed::default() })?;
        for _ in 0..completed {
            step(&mut pipeline, &mut worker)?;
        }
        server.finish()?;
        worker.cancel();
        assert!(worker.take_engine().is_err());
        retire(&mut pipeline, &mut worker)?;
        assert_unlocked(&mut pipeline, &fixture)?;
        let root = fixture.installation.path().to_owned();
        pipeline.run(
            TRUST_RESERVATION_BYTES + TRUST_OPERATION_RESERVATION_BYTES,
            move |context| {
                let trust = TrustStore::open(&root, [92; 32], &context)?;
                let state = trust.updater_selection(&context)?;
                assert!(state.accepted().is_none());
                assert!(!matches!(state.candidate(), Some(Candidate::Trial { .. })));
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn engine_requiring_another_engine_or_protocol_returns_installer_without_recursion() -> TestResult {
    for requirement in ["minimum_updater_release", "updater_protocol"] {
        let mut pipeline = Pipeline::new()?;
        let (fixture, _) = initialized(&mut pipeline)?;
        let (mut worker, server) = campaign(
            &mut pipeline,
            &fixture,
            Feed { requirement: Some(requirement), completed: 4, ..Feed::default() },
        )?;
        for _ in 0..3 {
            step(&mut pipeline, &mut worker)?;
        }
        assert_eq!(
            step(&mut pipeline, &mut worker)?.progress,
            Progress::Decision(Admission::Installer)
        );
        server.finish()?;
        assert!(submit(&mut pipeline, &mut worker).is_err());
        assert!(worker.take_engine().is_err());
        retire(&mut pipeline, &mut worker)?;
        assert_unlocked(&mut pipeline, &fixture)?;
    }
    Ok(())
}

#[test]
fn signed_engine_below_the_application_minimum_fails_before_download() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    let (mut worker, server) =
        campaign(&mut pipeline, &fixture, Feed { sequence: 1, completed: 4, ..Feed::default() })?;
    for _ in 0..3 {
        step(&mut pipeline, &mut worker)?;
    }
    submit(&mut pipeline, &mut worker)?;
    let observed = pipeline.observe(|| worker.poll())?;
    assert!(matches!(observed.outcome, JobOutcome::Failure(_)));
    assert_eq!(observed.progress, Progress::Failed);
    assert!(observed.failure.is_some());
    assert!(worker.take_engine().is_err());
    server.finish()?;
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}

#[test]
fn subscription_change_before_engine_selection_stops_the_original_campaign() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    let (mut worker, server) =
        campaign(&mut pipeline, &fixture, Feed { completed: 2, ..Feed::default() })?;
    step(&mut pipeline, &mut worker)?;
    step(&mut pipeline, &mut worker)?;
    server.finish()?;
    select_subscription(&fixture, Channel::Canary)?;
    submit(&mut pipeline, &mut worker)?;
    let observed = pipeline.observe(|| worker.poll())?;
    assert!(matches!(observed.outcome, JobOutcome::Failure(_)));
    assert_eq!(observed.progress, Progress::Failed);
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}

#[test]
fn canceled_staged_engine_resumes_the_same_generation_without_downloading_again() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    let (mut worker, server) = campaign(&mut pipeline, &fixture, Feed::default())?;
    for _ in 0..6 {
        step(&mut pipeline, &mut worker)?;
    }
    server.finish()?;
    retire(&mut pipeline, &mut worker)?;
    let root = fixture.installation.path().to_owned();
    let generation = pipeline.run(
        TRUST_RESERVATION_BYTES + TRUST_OPERATION_RESERVATION_BYTES,
        move |context| {
            let trust = TrustStore::open(&root, [92; 32], &context)?;
            Ok(trust
                .updater_selection(&context)?
                .candidate()
                .ok_or("Prepared engine absent")?
                .generation()
                .clone())
        },
    )?;
    let (mut resumed, server) =
        campaign(&mut pipeline, &fixture, Feed { completed: 4, ..Feed::default() })?;
    for _ in 0..6 {
        step(&mut pipeline, &mut resumed)?;
    }
    server.finish()?;
    let EngineUpdate::Prepared(prepared) = resumed.take_engine()? else {
        return Err("Unprobed engine became healthy".into());
    };
    assert_eq!(prepared.generation(), &generation);
    pipeline.run(4096, move |_| {
        drop(prepared);
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}
