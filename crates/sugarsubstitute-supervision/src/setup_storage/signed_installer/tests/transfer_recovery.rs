// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expire actual transfer workers at durable boundaries while keeping creator custody outside them.
use super::*;
use crate::application_launch::ApplicationLaunch;
use crate::setup_storage::test_worker::pipeline::Pipeline;

#[test]
fn expired_transfer_and_discarded_success_recover_with_the_original_creator() -> TestResult {
    for phase in ["before", "published", "snapshot", "returned"] {
        let mut pipeline = Pipeline::new()?;
        let (fixture, launch) = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
            let fixture = Fixture::new(&context)?;
            let prepared = fixture
                .import(SignedPackage::new(&fixture.archive, 1)?, &context)?
                .prepare(|| NOW, &context)?;
            let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
            let candidate = Arc::new(prepared.complete(&runtime, || NOW, &context)?);
            Ok((fixture, ApplicationLaunch::trial(candidate)))
        })?;
        let root = fixture.installation.path().to_owned();
        let record = root.join(".selection-state/.substitute-selection.json");
        let before = std::fs::read(&record)?;
        let owned = launch.clone();
        worker::run_reserved_expiring(SIGNED_TRANSFER_RESERVATION_BYTES, move |context, clock| {
            match phase {
                "before" => clock.expire(),
                "published" => clock.observe_checks(move || {
                    std::fs::read(&record)
                        .ok()
                        .and_then(|bytes| {
                            sugarsubstitute_contracts::installation::InstallationRecord::decode(
                                &bytes,
                            )
                            .ok()
                        })
                        .is_some_and(|record| {
                            matches!(record.candidate(), Some(Candidate::Trial { .. }))
                        })
                })?,
                "snapshot" => clock.observe_checks(move || {
                    std::fs::read_dir(root.join(".selection-state")).is_ok_and(|entries| {
                        entries.flatten().any(|entry| {
                            entry.file_name().to_string_lossy().starts_with(".trial-transfer")
                        })
                    })
                })?,
                _ => {}
            }
            let result = owned.prepare_trial(|| NOW, context);
            if phase == "returned" {
                result?;
                clock.expire();
            } else {
                assert!(result.is_err(), "Transfer did not stop at {phase}");
            }
            assert!(owned.retire().is_err());
            Ok(())
        })?;
        pipeline.run(RESERVATION_BYTES, move |context| {
            let store = Arc::new(SelectionStore::open(fixture.installation.path(), &context)?);
            let interrupted = store.load(&context)?.ok_or("Selection absent")?;
            if phase != "before" {
                assert!(matches!(interrupted.candidate(), Some(Candidate::Trial { .. })));
                assert!(InstallationRun::acquire(store.clone(), &context).is_err());
            }
            assert_eq!(
                launch.recover_and_retire(&context)?,
                crate::application_launch::CreationOutcome::NotCreated
            );
            let restored = store.load(&context)?.ok_or("Selection absent")?;
            assert!(matches!(restored.candidate(), Some(Candidate::Prepared { .. })));
            assert!(restored.rejected().is_empty());
            if phase == "before" {
                assert_eq!(restored.encode()?, before);
            } else {
                assert_eq!(restored.revision(), interrupted.revision() + 1);
            }
            assert_eq!(
                launch.recover_and_retire(&context)?,
                crate::application_launch::CreationOutcome::NotCreated
            );
            assert_eq!(store.load(&context)?, Some(restored));
            assert!(launch.prepare_trial(|| NOW, &context).is_err());
            assert!(launch.launch(&context).is_err());
            let _run = InstallationRun::acquire(store, &context)?;
            fixture.preserved()?;
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn creator_recovery_is_idempotent_and_cannot_undo_health_acceptance() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let prepared = fixture
            .import(SignedPackage::new(&fixture.archive, 1)?, context)?
            .prepare(|| NOW, context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
        let candidate = prepared.complete(&runtime, || NOW, context)?;
        let store = Arc::new(SelectionStore::open(fixture.installation.path(), context)?);
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let recovery = candidate.candidate.reserve_unstarted(&run, context)?;
        let admitted = Arc::clone(&candidate.candidate).begin_trial(
            &mut run,
            &candidate.offer.authorization(&candidate.trust),
            || NOW,
            context,
        )?;
        candidate.prepared().restore_unstarted(&recovery, context)?;
        candidate.prepared().restore_unstarted(&recovery, context)?;
        let restored = store.load(context)?.ok_or("Selection absent")?;
        drop(admitted);
        drop(run);
        // The original recovery still owns exclusion; a new run cannot reconstruct its authority.
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        drop(recovery);
        let mut later = InstallationRun::acquire(store.clone(), context)?;
        let recovery = later.reserve_unstarted(&restored, context)?;
        let trial = later.begin_trial(&restored, context)?;
        let accepted = later.accept(&trial, context)?;
        assert!(candidate.prepared().restore_unstarted(&recovery, context).is_err());
        assert_eq!(store.load(context)?, Some(accepted));
        fixture.preserved()?;
        Ok(())
    })
}
