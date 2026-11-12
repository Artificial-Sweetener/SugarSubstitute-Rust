// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual creator failures and independent cleanup jobs preserve resumable signed preparation.
use super::*;
use crate::{
    application_launch::{ApplicationLaunch, rejected_creation::RejectedCreation},
    setup_storage::test_worker::pipeline::Pipeline,
};

fn pending(context: &JobContext) -> Result<(Fixture, PathBuf, ApplicationLaunch), Failure> {
    let fixture = Fixture::new(context)?;
    let prepared = fixture
        .import(SignedPackage::new(&fixture.archive, 1)?, context)?
        .prepare(|| NOW, context)?;
    let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
    let candidate = prepared.complete(&runtime, || NOW, context)?;
    let path = candidate.directory().to_owned();
    let launch = ApplicationLaunch::trial(Arc::new(candidate));
    launch.prepare_trial(|| NOW, context)?;
    Ok((fixture, path, launch))
}

#[test]
fn unstarted_and_native_failed_creations_resume_the_same_signed_candidate_after_cleanup()
-> TestResult {
    for create in [false, true] {
        let mut pipeline = Pipeline::new()?;
        let (fixture, path, launch) =
            pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| pending(&context))?;
        let owned = launch.clone();
        pipeline.run(RESERVATION_BYTES, move |context| {
            if create {
                // The authenticated fixture contains inert bytes, which Windows refuses to execute.
                // The creation boundary reports that refusal without handing them to the OS loader.
                let creation = RejectedCreation::default();
                let cause = owned
                    .launch_rejected(&context, &creation)
                    .err()
                    .ok_or("Inert executable was started")?;
                assert!(cause.to_string().contains("application_launch_create"), "{cause}");
                assert_eq!(creation.attempts(), 1);
            }
            assert!(owned.retire().is_err());
            Ok(())
        })?;
        let owned = launch.clone();
        pipeline.run(RESERVATION_BYTES, move |context| {
            assert_eq!(
                owned.recover_and_retire(&context)?,
                crate::application_launch::CreationOutcome::NotCreated
            );
            assert_eq!(
                owned.recover_and_retire(&context)?,
                crate::application_launch::CreationOutcome::NotCreated
            );
            assert!(owned.launch(&context).is_err());
            Ok(())
        })?;
        pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
            let store = Arc::new(SelectionStore::open(fixture.installation.path(), &context)?);
            let snapshot = store.load(&context)?.ok_or("Selection absent")?;
            assert!(matches!(snapshot.candidate(), Some(Candidate::Prepared { .. })));
            assert!(snapshot.strikes().is_empty());
            let run = InstallationRun::acquire(store, &context)?;
            drop(run);
            let prepared = fixture
                .import(SignedPackage::new(&fixture.archive, 1)?, &context)?
                .prepare(|| NOW, &context)?;
            let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
            let candidate = prepared.complete(&runtime, || NOW, &context)?;
            assert_eq!(candidate.directory(), path);
            fixture.preserved()?;
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn cancelled_recovery_retains_trial_exclusion_until_an_independent_cleanup_job_finishes()
-> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _, launch) =
        pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| pending(&context))?;
    let owned = launch.clone();
    worker::run_reserved_expiring(RESERVATION_BYTES, move |context, clock| {
        clock.expire();
        assert!(owned.recover_and_retire(context).is_err());
        assert!(owned.retire().is_err());
        Ok(())
    })?;
    pipeline.run(RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(fixture.installation.path(), &context)?);
        let trial = store.load(&context)?.ok_or("Selection absent")?;
        assert!(matches!(trial.candidate(), Some(Candidate::Trial { .. })));
        assert!(InstallationRun::acquire(store.clone(), &context).is_err());
        assert_eq!(
            launch.recover_and_retire(&context)?,
            crate::application_launch::CreationOutcome::NotCreated
        );
        let resumed = store.load(&context)?.ok_or("Selection absent")?;
        assert!(matches!(resumed.candidate(), Some(Candidate::Prepared { .. })));
        assert_eq!(resumed.revision(), trial.revision() + 1);
        let _run = InstallationRun::acquire(store, &context)?;
        fixture.preserved()?;
        Ok(())
    })?;
    Ok(())
}
