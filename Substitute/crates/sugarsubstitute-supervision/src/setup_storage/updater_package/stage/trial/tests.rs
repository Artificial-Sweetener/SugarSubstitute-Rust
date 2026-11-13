// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise authenticated engine trial publication and withdrawal through real storage exclusion.
use super::*;
use crate::setup_storage::{
    self as storage,
    test_package::updater::{archive, evidence, run},
    test_worker::{self as worker, TestResult},
};
use std::sync::Arc;
use sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT;
use sugarsubstitute_update::fixture::{EXPIRES, NOW};

#[test]
fn abandoned_recovery_waits_for_the_trial_owner_and_not_for_witness_duplicates() -> TestResult {
    run(|context| {
        let root = worker::installation(149, context)?;
        let workspace = tempfile::tempdir()?;
        let mut trial = prepare(root.path(), workspace.path(), context)?.reserve_trial(context)?;
        trial.publish(|| NOW, context)?;
        // A witness duplicate left in another holder proves custody only while the owner lives.
        let duplicate = trial.prepared._preparation.witness().try_clone()?;
        let at = u64::try_from(NOW)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?
            .with_strike_clock(crate::setup_storage::selection::StrikeClock::Fixed(at));
        let pending = trust.updater_selection(context)?;
        assert!(trust.recover_updater_trial(context).is_err());
        assert_eq!(trust.updater_selection(context)?, pending);
        drop(trial);
        let recovered = trust.recover_updater_trial(context)?;
        let Some(Candidate::Trial { attempt }) = pending.candidate() else {
            return Err("Expected published trial".into());
        };
        // Recovery dates the strike with the injected clock, never the real one.
        assert_eq!(recovered, pending.reject(attempt, at)?);
        assert_eq!(recovered.strikes().iter().map(|strike| strike.at).collect::<Vec<_>>(), [at]);
        drop(duplicate);
        Ok(())
    })
}

#[test]
fn historical_preparation_only_engine_remains_staged_without_a_probe_trial() -> TestResult {
    run(|context| {
        let root = worker::installation(127, context)?;
        let workspace = tempfile::tempdir()?;
        let historical =
            br#"{"schema_version":1,"preparation_protocols":[1],"installation_schemas":[3]}"#;
        let (bytes, _) = archive(historical)?;
        let (trust, offer, download) = evidence(root.path(), &bytes, Component::Updater, context)?;
        let prepared = UpdaterPackage::import(
            root.path(),
            trust,
            offer,
            download,
            workspace.path(),
            || NOW,
            context,
        )?
        .stage(|| NOW, context)?;
        let before = prepared.source.trust.updater_selection(context)?;
        assert!(matches!(before.candidate(), Some(Candidate::Prepared { .. })));
        assert!(prepared.reserve_trial(context).is_err());
        let observer = TrustStore::open(root.path(), [92; 32], context)?;
        assert_eq!(observer.updater_selection(context)?, before);
        let _released = observer.claim_updater_preparation(context)?;
        Ok(())
    })
}

#[test]
fn lost_prepared_journal_cannot_be_reinitialized_during_trial_reservation() -> TestResult {
    run(|context| {
        let root = worker::installation(126, context)?;
        let workspace = tempfile::tempdir()?;
        let prepared = prepare(root.path(), workspace.path(), context)?;
        let directory = root.path().join("state/update-trust");
        for name in ["updater-selection-v1", "updater-established-v1"] {
            std::fs::remove_file(directory.join(name))?;
        }
        assert!(prepared.reserve_trial(context).is_err());
        for name in ["updater-selection-v1", "updater-established-v1"] {
            assert!(!directory.join(name).exists());
        }
        Ok(())
    })
}

fn prepare(
    root: &Path,
    workspace: &Path,
    context: &JobContext,
) -> Result<PreparedUpdater, Box<dyn std::error::Error + Send + Sync>> {
    let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
    let (trust, offer, download) = evidence(root, &bytes, Component::Updater, context)?;
    Ok(UpdaterPackage::import(root, trust, offer, download, workspace, || NOW, context)?
        .stage(|| NOW, context)?)
}

#[test]
fn publication_and_withdrawal_are_exact_idempotent_transitions_without_health() -> TestResult {
    run(|context| {
        let root = worker::installation(120, context)?;
        let workspace = tempfile::tempdir()?;
        let prepared = prepare(root.path(), workspace.path(), context)?;
        let mut trial = prepared.reserve_trial(context)?;
        let initial = trial.prepared.source.trust.updater_selection(context)?;
        trial.publish(|| NOW, context)?;
        let pending = trial.prepared.source.trust.updater_selection(context)?;
        assert!(
            matches!(pending.candidate(), Some(Candidate::Trial { attempt }) if attempt == trial.attempt())
        );
        assert_eq!(pending.accepted(), initial.accepted());
        let directory = Directory::open(&root.path().join("state/update-trust"), false)?;
        let pin = directory.read("updater-selection-v1")?;
        trial.publish(|| NOW, context)?;
        assert_eq!(trial.prepared.source.trust.updater_selection(context)?, pending);
        drop(pin);
        trial.restore(context)?;
        let restored = trial.prepared.source.trust.updater_selection(context)?;
        assert!(
            matches!(restored.candidate(), Some(Candidate::Prepared { generation }) if generation == &trial.attempt().generation)
        );
        assert_eq!(restored.accepted(), initial.accepted());
        assert!(restored.revision() > pending.revision());
        trial.restore(context)?;
        assert_eq!(trial.prepared.source.trust.updater_selection(context)?, restored);
        assert!(trial.publish(|| NOW, context).is_err());
        Ok(())
    })
}

#[test]
fn expired_or_backward_clock_cannot_publish_and_inert_withdrawal_remains_available() -> TestResult {
    run(|context| {
        let root = worker::installation(121, context)?;
        let workspace = tempfile::tempdir()?;
        let mut trial = prepare(root.path(), workspace.path(), context)?.reserve_trial(context)?;
        let initial = trial.prepared.source.trust.updater_selection(context)?;
        let mut calls = 0;
        assert!(
            trial
                .publish(
                    || {
                        calls += 1;
                        if calls == 1 { NOW } else { NOW - 1 }
                    },
                    context
                )
                .is_err()
        );
        assert_eq!(trial.prepared.source.trust.updater_selection(context)?, initial);
        assert!(trial.publish(|| EXPIRES, context).is_err());
        assert_eq!(trial.prepared.source.trust.updater_selection(context)?, initial);
        trial.restore(context)?;
        assert_eq!(trial.prepared.source.trust.updater_selection(context)?, initial);
        assert!(trial.publish(|| NOW, context).is_err());
        Ok(())
    })
}

#[test]
fn publication_and_withdrawal_storage_failures_keep_custody_and_support_exact_retry() -> TestResult
{
    run(|context| {
        let root = worker::installation(122, context)?;
        let workspace = tempfile::tempdir()?;
        let mut trial = prepare(root.path(), workspace.path(), context)?.reserve_trial(context)?;
        let directory = Directory::open(&root.path().join("state/update-trust"), false)?;
        let pin = directory.read("updater-selection-v1")?;
        let before = trial.prepared.source.trust.updater_selection(context)?;
        assert!(trial.publish(|| NOW, context).is_err());
        assert_eq!(trial.prepared.source.trust.updater_selection(context)?, before);
        assert!(trial.prepared.source.trust.claim_updater_preparation(context).is_err());
        drop(pin);
        trial.publish(|| NOW, context)?;
        let pending = trial.prepared.source.trust.updater_selection(context)?;
        let pin = directory.read("updater-selection-v1")?;
        assert!(trial.restore(context).is_err());
        assert_eq!(trial.prepared.source.trust.updater_selection(context)?, pending);
        assert!(trial.publish(|| NOW, context).is_err());
        drop(pin);
        trial.restore(context)?;
        assert!(matches!(
            trial.prepared.source.trust.updater_selection(context)?.candidate(),
            Some(Candidate::Prepared { .. })
        ));
        Ok(())
    })
}

#[test]
fn subscription_revocation_blocks_republication_but_not_unstarted_withdrawal() -> TestResult {
    run(|context| {
        let root = worker::installation(123, context)?;
        let workspace = tempfile::tempdir()?;
        let mut trial = prepare(root.path(), workspace.path(), context)?.reserve_trial(context)?;
        trial.publish(|| NOW, context)?;
        let trust = &trial.prepared.source.trust;
        let preference = trust.subscription(context)?;
        trust.select_subscription(
            preference,
            sugarsubstitute_contracts::update::Channel::Canary,
            context,
        )?;
        assert!(trial.publish(|| NOW, context).is_err());
        trial.restore(context)?;
        assert!(matches!(
            trial.prepared.source.trust.updater_selection(context)?.candidate(),
            Some(Candidate::Prepared { .. })
        ));
        Ok(())
    })
}

#[test]
fn corrupted_journal_is_preserved_and_cannot_reopen_creation_admission() -> TestResult {
    run(|context| {
        let root = worker::installation(124, context)?;
        let workspace = tempfile::tempdir()?;
        let mut trial = prepare(root.path(), workspace.path(), context)?.reserve_trial(context)?;
        trial.publish(|| NOW, context)?;
        let file = root.path().join("state/update-trust/updater-selection-v1");
        std::fs::write(&file, b"corrupt journal")?;
        assert!(trial.restore(context).is_err());
        assert!(trial.publish(|| NOW, context).is_err());
        assert_eq!(std::fs::read(&file)?, b"corrupt journal");
        assert!(trial.prepared.source.trust.claim_updater_preparation(context).is_err());
        Ok(())
    })
}

#[test]
fn engine_trial_pins_bytes_and_owns_only_engine_exclusion_until_retirement() -> TestResult {
    run(|context| {
        let root = worker::installation(125, context)?;
        let workspace = tempfile::tempdir()?;
        let mut trial = prepare(root.path(), workspace.path(), context)?.reserve_trial(context)?;
        trial.publish(|| NOW, context)?;
        let selection = Arc::new(storage::SelectionStore::open(root.path(), context)?);
        let before = selection.load(context)?;
        let _run = storage::InstallationRun::acquire(Arc::clone(&selection), context)?;
        let _setup = storage::transaction::admission::Admission::open(root.path(), false, context)?;
        let image = trial
            .prepared
            .source
            .trust
            .updater_path(&trial.attempt().generation)
            .join("app/sugarsubstitute-updater.exe");
        assert!(std::fs::write(&image, b"changed").is_err());
        assert!(trial.prepared.source.trust.claim_updater_preparation(context).is_err());
        assert_eq!(selection.load(context)?, before);
        trial.restore(context)?;
        drop(trial);
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let _preparation = trust.claim_updater_preparation(context)?;
        std::fs::write(&image, b"custody retired")?;
        assert_eq!(selection.load(context)?, before);
        Ok(())
    })
}
