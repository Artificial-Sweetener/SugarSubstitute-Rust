// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover real protected journals and verify native exclusion, preservation and authenticated retries.
use super::*;
use crate::setup_storage::{
    self as storage, UnstartedUpdaterTrial, UpdaterPackage,
    test_package::updater::{archive, evidence_intent, run},
    test_worker::{self as worker, TestResult},
};
use std::sync::Arc;
use sugarsubstitute_contracts::{
    application_launch::updater::CURRENT_UPDATER_CONTRACT,
    installation::{GenerationId, PreparationIntent},
};
use sugarsubstitute_update::fixture::NOW;

/// Exercise the authenticated import/stage path rather than editing candidate records directly.
fn package(
    root: &Path,
    workspace: &Path,
    contract: &[u8],
    version: u32,
    intent: PreparationIntent,
    context: &JobContext,
) -> Result<UpdaterPackage, Box<dyn std::error::Error + Send + Sync>> {
    let (bytes, _) = archive(contract)?;
    let (trust, offer, download) =
        evidence_intent(root, &bytes, Component::Updater, version, intent, context)?;
    Ok(UpdaterPackage::import(root, trust, offer, download, workspace, || NOW, context)?)
}

fn trial(
    root: &Path,
    workspace: &Path,
    context: &JobContext,
) -> Result<UnstartedUpdaterTrial, Box<dyn std::error::Error + Send + Sync>> {
    let mut trial = package(
        root,
        workspace,
        CURRENT_UPDATER_CONTRACT,
        2,
        PreparationIntent::Automatic,
        context,
    )?
    .stage(|| NOW, context)?
    .reserve_trial(context)?;
    trial.publish(|| NOW, context)?;
    Ok(trial)
}

#[test]
fn recovery_preserves_history_user_files_and_damaged_candidate_without_application_locks()
-> TestResult {
    run(|context| {
        let root = worker::installation(150, context)?;
        let workspace = tempfile::tempdir()?;
        let seed = package(
            root.path(),
            workspace.path(),
            CURRENT_UPDATER_CONTRACT,
            2,
            PreparationIntent::Automatic,
            context,
        )?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let mut history = trust.updater_selection(context)?;
        // This seeds historical selection only; inert fixtures never claim native health proof.
        for number in [1, 2] {
            let generation = Generation {
                id: GenerationId::new(format!("{number:032x}"))?,
                package: PackageDigest::new(format!("{number:064x}"))?,
                release: sugarsubstitute_contracts::installation::SignedRelease::new(
                    format!("1.0.{number}"),
                    number + 1,
                )?,
            };
            history = history
                .prepare(
                    generation.clone(),
                    PreparationIntent::Automatic,
                    crate::setup_storage::selection::unix_now()?,
                )?
                .prepared(&generation)?
                .begin_trial()?;
            let Some(Candidate::Trial { attempt }) = history.candidate() else {
                return Err("Historical fixture trial absent".into());
            };
            history = history.accept(attempt)?;
        }
        trust.protected(context, |protection| {
            let _writer = trust.writer(protection, context)?;
            trust.write_updater(&history, protection, context)
        })?;
        let mut trial = seed.stage(|| NOW, context)?.reserve_trial(context)?;
        trial.publish(|| NOW, context)?;
        let generation = trial.attempt().generation.clone();
        let pending = trust.updater_selection(context)?;
        drop(trial);
        let payload = trust.updater_path(&generation).join("app/sugarsubstitute-updater.exe");
        std::fs::write(&payload, b"damaged candidate")?;
        let receipt = trust.root.path().join(TrustStore::updater_receipt_name(&generation));
        let receipt_bytes = std::fs::read(&receipt)?;
        let checkpoint = std::fs::read(trust.root.path().join("checkpoint"))?;
        let user_file = root.path().join("user-settings.json");
        std::fs::write(&user_file, b"preserve authored settings")?;
        let app = Arc::new(storage::SelectionStore::open(root.path(), context)?);
        let app_before = app.load(context)?;
        let _run = storage::InstallationRun::acquire(app.clone(), context)?;
        let _setup = storage::transaction::admission::Admission::open(root.path(), false, context)?;
        let recovered = trust.recover_updater_trial(context)?;
        assert_eq!(recovered.accepted(), history.accepted());
        assert_eq!(recovered.retained(), history.retained());
        assert!(recovered.candidate().is_none());
        assert_eq!(recovered.revision(), pending.revision() + 1);
        assert_eq!(std::fs::read(&payload)?, b"damaged candidate");
        assert_eq!(std::fs::read(receipt)?, receipt_bytes);
        assert_eq!(std::fs::read(trust.root.path().join("checkpoint"))?, checkpoint);
        assert_eq!(std::fs::read(user_file)?, b"preserve authored settings");
        assert_eq!(app.load(context)?, app_before);
        let _pin = trust.root.read(SELECTION)?;
        assert_eq!(trust.recover_updater_trial(context)?, recovered);
        Ok(())
    })
}

#[test]
fn a_second_abandoned_trial_withholds_the_digest_and_explicit_repair_restages_it() -> TestResult {
    run(|context| {
        let root = worker::installation(151, context)?;
        let workspace = tempfile::tempdir()?;
        // One abandoned engine trial is one strike; automatic staging still retries it.
        drop(trial(root.path(), workspace.path(), context)?);
        let trial = trial(root.path(), workspace.path(), context)?;
        let attempt = trial.attempt().clone();
        drop(trial);
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        assert!(
            package(
                root.path(),
                workspace.path(),
                CURRENT_UPDATER_CONTRACT,
                2,
                PreparationIntent::Automatic,
                context
            )?
            .stage(|| NOW, context)
            .is_err()
        );
        let withheld = trust.updater_selection(context)?;
        assert!(withheld.candidate().is_none());
        assert_eq!(withheld.rollback(), Some(&attempt.generation));
        assert!(
            withheld.withheld(
                &attempt.generation.package,
                crate::setup_storage::selection::unix_now()?
            )
        );
        let repaired = package(
            root.path(),
            workspace.path(),
            CURRENT_UPDATER_CONTRACT,
            2,
            PreparationIntent::Repair,
            context,
        )?
        .stage(|| NOW, context)?;
        assert_ne!(repaired.generation().id, attempt.generation.id);
        assert_eq!(repaired.generation().package, attempt.generation.package);
        assert!(trust.updater_path(&attempt.generation).is_dir());
        assert!(
            matches!(trust.updater_selection(context)?.candidate(), Some(Candidate::Prepared { generation }) if generation == repaired.generation())
        );
        Ok(())
    })
}

#[test]
fn newer_authenticated_payload_recovers_abandoned_trial_during_normal_staging() -> TestResult {
    run(|context| {
        let root = worker::installation(152, context)?;
        let workspace = tempfile::tempdir()?;
        let trial = trial(root.path(), workspace.path(), context)?;
        let previous = trial.attempt().generation.clone();
        drop(trial);
        let newer = br#"{"schema_version":2,"preparation_protocols":[1,2],"installation_schemas":[3],"probe_protocols":[2]}"#;
        let staged = package(
            root.path(),
            workspace.path(),
            newer,
            3,
            PreparationIntent::Automatic,
            context,
        )?
        .stage(|| NOW, context)?;
        assert_ne!(staged.generation().package, previous.package);
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let state = trust.updater_selection(context)?;
        assert!(state.accepted().is_none());
        assert!(
            matches!(state.candidate(), Some(Candidate::Prepared { generation }) if generation == staged.generation())
        );
        assert!(trust.updater_path(&previous).is_dir());
        Ok(())
    })
}

#[test]
fn failed_publication_retains_trial_and_releases_custody_for_exact_retry() -> TestResult {
    run(|context| {
        let root = worker::installation(153, context)?;
        let workspace = tempfile::tempdir()?;
        drop(trial(root.path(), workspace.path(), context)?);
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let pending = trust.updater_selection(context)?;
        let pin = trust.root.read(SELECTION)?;
        assert!(trust.recover_updater_trial(context).is_err());
        assert_eq!(trust.updater_selection(context)?, pending);
        drop(trust.claim_updater_preparation(context)?);
        drop(pin);
        let recovered = trust.recover_updater_trial(context)?;
        assert!(recovered.candidate().is_none());
        assert_eq!(recovered.revision(), pending.revision() + 1);
        Ok(())
    })
}

#[test]
fn missing_or_corrupt_continuity_is_preserved_and_never_initialized_by_recovery() -> TestResult {
    run(|context| {
        for (name, missing) in
            [(MARKER, true), (MARKER, false), (SELECTION, true), (SELECTION, false)]
        {
            let root = worker::installation(154, context)?;
            let workspace = tempfile::tempdir()?;
            drop(trial(root.path(), workspace.path(), context)?);
            let trust = TrustStore::open(root.path(), [92; 32], context)?;
            let path = trust.root.path().join(name);
            if missing {
                std::fs::remove_file(&path)?;
            } else {
                std::fs::write(&path, b"unrecognized authoritative state")?;
            }
            assert!(trust.recover_updater_trial(context).is_err());
            if missing {
                assert!(!path.exists());
            } else {
                assert_eq!(std::fs::read(path)?, b"unrecognized authoritative state");
            }
            drop(trust.claim_updater_preparation(context)?);
        }
        Ok(())
    })
}

#[test]
fn cancellation_after_fresh_exclusion_preserves_trial_and_releases_the_native_handle() -> TestResult
{
    let root_holder = Arc::new(std::sync::Mutex::new(None));
    let output = root_holder.clone();
    worker::run_reserved_expiring(
        storage::SIGNED_INSTALLER_RESERVATION_BYTES + storage::UPDATER_STAGING_RESERVATION_BYTES,
        move |context, clock| {
            let root = worker::installation(155, context)?;
            let workspace = tempfile::tempdir()?;
            drop(trial(root.path(), workspace.path(), context)?);
            let trust = TrustStore::open(root.path(), [92; 32], context)?;
            let selection_path = trust.root.path().join(SELECTION);
            let before = std::fs::read(&selection_path)?;
            let rendezvous = trust.root.path().join("updater-preparation-v1");
            let observed = rendezvous.clone();
            // Expire only once the real fresh acquisition holds its exclusion slot.
            clock.observe_checks(move || {
                crate::durable_storage::protection::exclusion_held(&observed).unwrap_or(false)
            })?;
            assert!(trust.recover_updater_trial(context).is_err());
            assert_eq!(std::fs::read(selection_path)?, before);
            assert!(std::fs::read(rendezvous)?.is_empty());
            *crate::registry::lock(&output) = Some(root);
            Ok(())
        },
    )?;
    let root = crate::registry::lock(&root_holder).take().ok_or("Fixture root absent")?;
    run(move |context| {
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        assert!(trust.recover_updater_trial(context)?.candidate().is_none());
        Ok(())
    })
}
