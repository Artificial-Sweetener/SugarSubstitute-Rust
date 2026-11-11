// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Cached signed candidates use the real trial creator and restore selection when no process starts.
use super::*;
use crate::application_launch::{
    ApplicationLaunch, CREATION_RESERVATION_BYTES, CreationOutcome, RECOVERY_RESERVATION_BYTES,
};
use sugarsubstitute_contracts::installation::Candidate;

#[test]
fn cached_trial_channel_and_running_engine_guard_share_one_admission_ledger() -> TestResult {
    let (fixture, _) = restart::prepared()?;
    let mut pipeline = Pipeline::new()?;
    let guard = pipeline.run(RESERVATION_BYTES + TRUST_OPERATION_RESERVATION_BYTES, |context| {
        Ok(context.memory().claim(RESERVATION_BYTES)?)
    })?;
    let mut worker = Worker::resume_cached(cached_configuration(&fixture))?;
    for expected in [Progress::Restoring, Progress::Reopening, Progress::Ready] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    trial_with_engine_guard(&mut pipeline, &mut worker, guard)?;
    assert_unlocked(&mut pipeline, &fixture)
}

#[test]
fn online_reopened_trial_retires_metadata_buffers_before_launch_admission() -> TestResult {
    let (fixture, _) = restart::prepared()?;
    let mut pipeline = Pipeline::new()?;
    let guard = pipeline.run(RESERVATION_BYTES + TRUST_OPERATION_RESERVATION_BYTES, |context| {
        Ok(context.memory().claim(RESERVATION_BYTES)?)
    })?;
    let signed = SignedPackage::new(&fixture.archive, 1)?;
    let (mut worker, server) = campaign(&mut pipeline, &fixture, signed, false)?;
    for expected in
        [Progress::Checking, Progress::Reopening, Progress::Remembering, Progress::Ready]
    {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    server.finish()?;
    trial_with_engine_guard(&mut pipeline, &mut worker, guard)?;
    assert_unlocked(&mut pipeline, &fixture)
}

/// Exercise launch and recovery while engine pins, candidate and channel share the real ledger.
fn trial_with_engine_guard(
    pipeline: &mut Pipeline,
    worker: &mut Worker,
    guard: sugarsubstitute_contracts::execution::MemoryClaim,
) -> TestResult {
    use sugarsubstitute_execution::activation_channel::PreparedChannel;
    let launch = ApplicationLaunch::trial(Arc::from(worker.take_candidate()?));
    let held = launch.clone();
    pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        held.prepare_trial(|| NOW, &context)?;
        Ok(())
    })?;
    let held = launch.clone();
    let (mut channel, offer) =
        pipeline.run(crate::application_launch::ACTIVATION_RESERVATION_BYTES, move |context| {
            let channel = PreparedChannel::prepare(&context)?;
            let offer = held.offer(channel.identity(), &context)?;
            Ok((channel, offer))
        })?;
    pipeline.run(CREATION_RESERVATION_BYTES, |_| Ok(()))?;
    pipeline.run(RECOVERY_RESERVATION_BYTES, move |context| {
        channel.retire_child()?;
        assert!(channel.endpoint().retire(&context)?);
        assert_eq!(launch.recover_and_retire(&context)?, CreationOutcome::NotCreated);
        drop(offer);
        drop(guard);
        Ok(())
    })?;
    Ok(())
}

#[test]
fn offline_candidate_transfers_trial_custody_and_restores_unstarted_selection() -> TestResult {
    let (fixture, original) = restart::prepared()?;
    let mut pipeline = Pipeline::new()?;
    let mut worker = Worker::resume_cached(cached_configuration(&fixture))?;
    for expected in [Progress::Restoring, Progress::Reopening, Progress::Ready] {
        assert_eq!(step(&mut pipeline, &mut worker)?.progress, expected);
    }
    let candidate = worker.take_candidate()?;
    assert_eq!(candidate.directory(), original);
    let launch = ApplicationLaunch::trial(Arc::from(candidate));
    let root = fixture.installation.path().to_owned();
    let held = launch.clone();
    pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        held.prepare_trial(|| NOW, &context)?;
        let store = Arc::new(SelectionStore::open(&root, &context)?);
        let selection = store.load(&context)?.ok_or("Selection absent")?;
        assert!(matches!(selection.candidate(), Some(Candidate::Trial { .. })));
        assert!(InstallationRun::acquire(store, &context).is_err());
        Ok(())
    })?;
    pipeline.run(LAUNCH_RESERVATION_BYTES, |_| Ok(()))?;
    pipeline.run(CREATION_RESERVATION_BYTES, |_| Ok(()))?;
    pipeline.run(RECOVERY_RESERVATION_BYTES, move |context| {
        assert_eq!(launch.recover_and_retire(&context)?, CreationOutcome::NotCreated);
        assert_eq!(launch.recover_and_retire(&context)?, CreationOutcome::NotCreated);
        Ok(())
    })?;
    let root = fixture.installation.path().to_owned();
    pipeline.run(RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(&root, &context)?);
        let selection = store.load(&context)?.ok_or("Selection absent")?;
        assert!(matches!(selection.candidate(), Some(Candidate::Prepared { .. })));
        assert!(InstallationRun::acquire(store, &context).is_ok());
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)
}

#[test]
fn busy_application_refuses_cached_trial_without_changing_selection_or_claiming_creation()
-> TestResult {
    let (fixture, _) = restart::prepared()?;
    let mut pipeline = Pipeline::new()?;
    let mut worker = Worker::resume_cached(cached_configuration(&fixture))?;
    for _ in 0..3 {
        step(&mut pipeline, &mut worker)?;
    }
    let launch = ApplicationLaunch::trial(Arc::from(worker.take_candidate()?));
    let root = fixture.installation.path().to_owned();
    pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(&root, &context)?);
        let run = InstallationRun::acquire(store.clone(), &context)?;
        let original = store.load(&context)?;
        assert!(launch.prepare_trial(|| NOW, &context).is_err());
        assert_eq!(launch.recover_and_retire(&context)?, CreationOutcome::NotCreated);
        assert_eq!(store.load(&context)?, original);
        assert!(InstallationRun::acquire(store.clone(), &context).is_err());
        drop(run);
        assert!(InstallationRun::acquire(store, &context).is_ok());
        Ok(())
    })?;
    assert_unlocked(&mut pipeline, &fixture)
}
