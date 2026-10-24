// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retry native creation with the same pinned candidate after exact unstarted recovery.
use super::*;
use crate::application_launch::ApplicationLaunch;
use crate::setup_storage::test_worker::pipeline::Pipeline;

#[test]
fn recovered_candidate_retries_creation_without_reimport_and_rechecks_expiry_and_keys() -> TestResult
{
    let mut pipeline = Pipeline::new()?;
    let (fixture, candidate) = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
        let fixture = Fixture::new(&context)?;
        let prepared = fixture
            .import(SignedPackage::new(&fixture.archive, 1)?, &context)?
            .prepare(|| NOW, &context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
        let candidate = Arc::new(prepared.complete(&runtime, || NOW, &context)?);
        Ok((fixture, candidate))
    })?;
    let directory = candidate.directory().to_owned();
    for _ in 0..3 {
        let launch = ApplicationLaunch::trial(candidate.clone());
        let owned = launch.clone();
        pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
            owned.prepare_trial(|| NOW, &context)?;
            Ok(())
        })?;
        let owned = launch.clone();
        pipeline.run(RESERVATION_BYTES, move |context| {
            let cause = owned.launch(&context).err().ok_or("Inert payload was started")?;
            assert_eq!(cause.operation(), "application_launch_create");
            Ok(())
        })?;
        pipeline.run(RESERVATION_BYTES, move |context| {
            assert_eq!(
                launch.recover_and_retire(&context)?,
                crate::application_launch::CreationOutcome::NotCreated
            );
            assert_eq!(
                launch.recover_and_retire(&context)?,
                crate::application_launch::CreationOutcome::NotCreated
            );
            assert!(launch.prepare_trial(|| NOW, &context).is_err());
            Ok(())
        })?;
        assert_eq!(candidate.directory(), directory);
        assert!(std::fs::write(directory.join("sugarsubstitute.exe"), b"changed").is_err());
    }
    for rotate in [false, true] {
        let installation = fixture.installation.path().to_owned();
        let launch = ApplicationLaunch::trial(candidate.clone());
        let authority = candidate.clone();
        pipeline.run(RESERVATION_BYTES, move |context| {
            let store = Arc::new(SelectionStore::open(&installation, &context)?);
            let before = store.load(&context)?.ok_or("Selection absent")?;
            assert!(matches!(before.candidate(), Some(Candidate::Prepared { .. })));
            assert!(before.rejected().is_empty());
            if rotate {
                authority.trust.rotate(&root(&key(93)?, 2, Some(&key(92)?))?, &context)?;
            }
            assert!(launch.prepare_trial(|| if rotate { NOW } else { EXPIRES }, &context).is_err());
            assert_eq!(
                launch.recover_and_retire(&context)?,
                crate::application_launch::CreationOutcome::NotCreated
            );
            assert_eq!(store.load(&context)?, Some(before));
            let _run = InstallationRun::acquire(store, &context)?;
            Ok(())
        })?;
    }
    fixture.preserved()?;
    drop(candidate);
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))?;
    Ok(())
}
