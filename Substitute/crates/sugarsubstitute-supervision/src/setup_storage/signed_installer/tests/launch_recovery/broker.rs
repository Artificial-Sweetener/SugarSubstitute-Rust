// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Broker outcomes control recovery of the real authenticated trial and its native exclusion.
use super::*;
use crate::application_launch::{CreationOutcome, broker::TRANSFER_RESERVATION_BYTES};

/// Model only the separate creator reply; publication, file custody and recovery remain real.
fn response(disposition: &str) -> Result<Vec<u8>, Failure> {
    Ok(serde_json::to_vec(
        &serde_json::json!({"version":1,"id":vec![7;32],"disposition":disposition}),
    )?)
}

#[test]
fn broker_negative_reply_restores_the_exact_trial_once_after_retaining_unanswered_custody()
-> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, path, launch) =
        pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| pending(&context))?;
    pipeline.run(TRANSFER_RESERVATION_BYTES + RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(fixture.installation.path(), &context)?);
        let trial = store.load(&context)?.ok_or("Trial absent")?;
        let offer = launch.offer([7; 32], &context)?;
        let frame: serde_json::Value = serde_json::from_slice(offer.frame(&context)?)?;
        assert_eq!(frame["entry"]["kind"], "trial");
        assert_ne!(frame["entry"]["run"], frame["entry"]["snapshot"]);
        assert!(!frame["pins"].as_array().ok_or("Pins absent")?.is_empty());
        assert!(launch.recover_and_retire(&context).is_err());
        assert_eq!(store.load(&context)?, Some(trial.clone()));
        assert!(InstallationRun::acquire(store.clone(), &context).is_err());
        assert!(std::fs::write(path.join("sugarsubstitute.exe"), b"replace").is_err());
        assert_eq!(
            offer.observe_result(&response("not_created")?, &context)?,
            CreationOutcome::NotCreated
        );
        assert!(launch.retire().is_err());
        assert_eq!(launch.recover_and_retire(&context)?, CreationOutcome::NotCreated);
        let restored = store.load(&context)?.ok_or("Restored selection absent")?;
        assert!(matches!(restored.candidate(), Some(Candidate::Prepared { .. })));
        assert_eq!(restored.revision(), trial.revision() + 1);
        assert_eq!(launch.recover_and_retire(&context)?, CreationOutcome::NotCreated);
        assert_eq!(store.load(&context)?, Some(restored));
        let _run = InstallationRun::acquire(store, &context)?;
        fixture.preserved()?;
        Ok(())
    })
}

#[test]
fn created_or_indeterminate_broker_results_cannot_restore_an_unstarted_trial() -> TestResult {
    for (disposition, outcome) in
        [("created", CreationOutcome::Created), ("indeterminate", CreationOutcome::Indeterminate)]
    {
        let mut pipeline = Pipeline::new()?;
        let (fixture, _, launch) =
            pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| pending(&context))?;
        pipeline.run(TRANSFER_RESERVATION_BYTES + RESERVATION_BYTES, move |context| {
            let store = Arc::new(SelectionStore::open(fixture.installation.path(), &context)?);
            let trial = store.load(&context)?.ok_or("Trial absent")?;
            let offer = launch.offer([7; 32], &context)?;
            offer.frame(&context)?;
            assert_eq!(offer.observe_result(&response(disposition)?, &context)?, outcome);
            assert!(offer.observe_result(&response("not_created")?, &context).is_err());
            assert_eq!(launch.recover_and_retire(&context)?, outcome);
            assert_eq!(store.load(&context)?, Some(trial));
            assert!(launch.launch(&context).is_err());
            fixture.preserved()?;
            Ok(())
        })?;
    }
    Ok(())
}
