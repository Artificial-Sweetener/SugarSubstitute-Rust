// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify independent updater preparation, restart continuity and native custody using inert packages.
use super::*;
use crate::setup_storage::{
    self as storage, UpdaterPackage,
    test_package::updater::{archive, evidence, run},
    test_worker::{self as worker, TestResult},
};
use std::sync::Arc;

#[test]
fn resuming_the_same_engine_preserves_the_existing_release_history_file() -> TestResult {
    run(|context| {
        let root = worker::installation(110, context)?;
        let workspace = tempfile::tempdir()?;
        let prepared = imported(root.path(), workspace.path(), context)?.stage(|| NOW, context)?;
        let generation = prepared.generation().clone();
        drop(prepared);
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let history = trust.root.read("releases")?;
        let original = std::fs::read(trust.root.path().join("releases"))?;
        let resumed = imported(root.path(), workspace.path(), context)?.stage(|| NOW, context)?;
        assert_eq!(resumed.generation(), &generation);
        assert_eq!(std::fs::read(trust.root.path().join("releases"))?, original);
        drop(resumed);
        // The pinned file denies replacement: a new signed release must still commit its floor.
        let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
        let (current, offer, download) = storage::test_package::updater::evidence_version(
            root.path(),
            &bytes,
            Component::Updater,
            3,
            context,
        )?;
        let package = UpdaterPackage::import(
            root.path(),
            current,
            offer,
            download,
            workspace.path(),
            || NOW,
            context,
        )?;
        assert!(package.stage(|| NOW, context).is_err());
        assert_eq!(std::fs::read(trust.root.path().join("releases"))?, original);
        assert_eq!(
            trust.updater_selection(context)?.candidate().map(Candidate::generation),
            Some(&generation)
        );
        drop(history);
        Ok(())
    })
}

#[test]
fn explicit_engine_repair_uses_a_fresh_generation_and_preserves_damaged_bytes() -> TestResult {
    run(|context| {
        let root = worker::installation(107, context)?;
        let workspace = tempfile::tempdir()?;
        let prepared = imported(root.path(), workspace.path(), context)?.stage(|| NOW, context)?;
        let previous = prepared.generation().clone();
        drop(prepared);
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let old_file = trust.updater_path(&previous).join("app/sugarsubstitute-updater.exe");
        std::fs::write(&old_file, b"damaged")?;
        std::fs::remove_file(trust.root.path().join(TrustStore::updater_receipt_name(&previous)))?;
        drop(trust);
        let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
        let (trust, offer, download) = storage::test_package::updater::evidence_intent(
            root.path(),
            &bytes,
            Component::Updater,
            2,
            sugarsubstitute_contracts::installation::PreparationIntent::Repair,
            context,
        )?;
        let repaired = UpdaterPackage::import(
            root.path(),
            trust,
            offer,
            download,
            workspace.path(),
            || NOW,
            context,
        )?
        .stage(|| NOW, context)?;
        assert_ne!(repaired.generation().id, previous.id);
        assert_eq!(std::fs::read(old_file)?, b"damaged");
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        assert_eq!(
            std::fs::read(
                trust.updater_path(repaired.generation()).join("app/sugarsubstitute-updater.exe")
            )?,
            b"inert engine"
        );
        assert!(trust.updater_selection(context)?.accepted().is_none());
        Ok(())
    })
}

#[test]
fn engine_records_cannot_be_substituted_between_installations_or_record_domains() -> TestResult {
    run(|context| {
        let first = worker::installation(108, context)?;
        let second = worker::installation(109, context)?;
        let workspace = tempfile::tempdir()?;
        drop(imported(first.path(), workspace.path(), context)?.stage(|| NOW, context)?);
        drop(imported(second.path(), workspace.path(), context)?.stage(|| NOW, context)?);
        let first = TrustStore::open(first.path(), [92; 32], context)?;
        let second = TrustStore::open(second.path(), [92; 32], context)?;
        for source in
            [first.root.path().join(SELECTION), second.root.path().join("subscription-v1")]
        {
            let bytes = std::fs::read(source)?;
            let destination = second.root.path().join(SELECTION);
            std::fs::write(&destination, &bytes)?;
            assert!(second.updater_selection(context).is_err());
            assert_eq!(std::fs::read(destination)?, bytes);
        }
        Ok(())
    })
}

#[test]
fn newer_release_supersedes_preparation_even_when_payload_bytes_are_identical() -> TestResult {
    run(|context| {
        let root = worker::installation(105, context)?;
        let workspace = tempfile::tempdir()?;
        let original = imported(root.path(), workspace.path(), context)?.stage(|| NOW, context)?;
        let previous = original.generation().clone();
        drop(original);
        let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
        let (trust, offer, download) = storage::test_package::updater::evidence_version(
            root.path(),
            &bytes,
            Component::Updater,
            3,
            context,
        )?;
        let current = UpdaterPackage::import(
            root.path(),
            trust,
            offer,
            download,
            workspace.path(),
            || NOW,
            context,
        )?
        .stage(|| NOW, context)?;
        assert_ne!(current.generation().id, previous.id);
        assert_eq!(current.generation().package, previous.package);
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        assert_eq!(
            std::fs::read(trust.updater_path(&previous).join("app/sugarsubstitute-updater.exe"))?,
            b"inert engine"
        );
        assert!(
            matches!(trust.updater_selection(context)?.candidate(), Some(Candidate::Prepared { generation }) if generation == current.generation())
        );
        Ok(())
    })
}

#[test]
fn missing_release_receipt_never_relabels_a_prepared_generation() -> TestResult {
    run(|context| {
        let root = worker::installation(106, context)?;
        let workspace = tempfile::tempdir()?;
        let original = imported(root.path(), workspace.path(), context)?.stage(|| NOW, context)?;
        let generation = original.generation().clone();
        drop(original);
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        let receipt = trust.root.path().join(TrustStore::updater_receipt_name(&generation));
        std::fs::remove_file(&receipt)?;
        drop(trust);
        assert!(imported(root.path(), workspace.path(), context)?.stage(|| NOW, context).is_err());
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        assert_eq!(trust.updater_selection(context)?, before);
        assert!(!receipt.exists());
        Ok(())
    })
}
use sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT;
use sugarsubstitute_update::fixture::{EXPIRES, NOW};

fn imported(
    root: &Path,
    workspace: &Path,
    context: &JobContext,
) -> Result<UpdaterPackage, Box<dyn std::error::Error + Send + Sync>> {
    let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
    let (trust, offer, download) = evidence(root, &bytes, Component::Updater, context)?;
    Ok(UpdaterPackage::import(root, trust, offer, download, workspace, || NOW, context)?)
}

#[test]
fn staged_engine_survives_restart_and_resumes_without_changing_application_selection() -> TestResult
{
    run(|context| {
        let root = worker::installation(101, context)?;
        let workspace = tempfile::tempdir()?;
        let app = Arc::new(storage::SelectionStore::open(root.path(), context)?);
        let original = app.load(context)?;
        let _run = storage::InstallationRun::acquire(app.clone(), context)?;
        let _setup = storage::transaction::admission::Admission::open(root.path(), false)?;
        let prepared = imported(root.path(), workspace.path(), context)?.stage(|| NOW, context)?;
        let generation = prepared.generation().clone();
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let state = trust.updater_selection(context)?;
        assert!(
            matches!(state.candidate(), Some(Candidate::Prepared { generation: candidate }) if candidate == &generation)
        );
        assert_eq!(state.accepted(), None);
        assert_eq!(app.load(context)?, original);
        assert_eq!(prepared.digest(), generation.package.as_str());
        assert!(trust.claim_updater_preparation(context).is_err());
        let executable = trust.updater_path(&generation).join("app/sugarsubstitute-updater.exe");
        assert!(std::fs::write(&executable, b"changed").is_err());
        drop(prepared);
        drop(trust);
        let resumed = imported(root.path(), workspace.path(), context)?.stage(|| NOW, context)?;
        assert_eq!(resumed.generation(), &generation);
        let reopened = TrustStore::open(root.path(), [92; 32], context)?;
        assert_eq!(reopened.updater_selection(context)?, state);
        assert_eq!(std::fs::read(executable)?, b"inert engine");
        assert_eq!(app.load(context)?, original);
        Ok(())
    })
}

#[test]
fn expired_final_authority_leaves_staging_recoverable_and_never_accepts_an_engine() -> TestResult {
    run(|context| {
        let root = worker::installation(102, context)?;
        let workspace = tempfile::tempdir()?;
        let package = imported(root.path(), workspace.path(), context)?;
        let mut calls = 0;
        let result = package.stage(
            || {
                calls += 1;
                if calls <= 3 { NOW } else { EXPIRES }
            },
            context,
        );
        assert!(result.is_err());
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let state = trust.updater_selection(context)?;
        assert!(matches!(state.candidate(), Some(Candidate::Staging { .. })));
        assert!(state.accepted().is_none());
        let _released = trust.claim_updater_preparation(context)?;
        Ok(())
    })
}

#[test]
fn corrupt_or_missing_established_engine_journal_is_preserved_and_not_reinitialized() -> TestResult
{
    run(|context| {
        for missing in [false, true] {
            let root = worker::installation(103, context)?;
            let workspace = tempfile::tempdir()?;
            let prepared =
                imported(root.path(), workspace.path(), context)?.stage(|| NOW, context)?;
            drop(prepared);
            let trust = TrustStore::open(root.path(), [92; 32], context)?;
            let path = trust.root.path().join(SELECTION);
            if missing {
                std::fs::remove_file(&path)?;
            } else {
                std::fs::write(&path, b"corrupt engine selection")?;
            }
            assert!(trust.updater_selection(context).is_err());
            if missing {
                assert!(!path.exists());
            } else {
                assert_eq!(std::fs::read(&path)?, b"corrupt engine selection");
            }
            assert!(storage::SelectionStore::open(root.path(), context)?.load(context)?.is_some());
        }
        Ok(())
    })
}

#[test]
fn changed_staged_payload_fails_resume_without_overwriting_bytes_or_selection() -> TestResult {
    run(|context| {
        let root = worker::installation(104, context)?;
        let workspace = tempfile::tempdir()?;
        let prepared = imported(root.path(), workspace.path(), context)?.stage(|| NOW, context)?;
        let generation = prepared.generation().clone();
        drop(prepared);
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        let path = trust.updater_path(&generation).join("app/sugarsubstitute-updater.exe");
        std::fs::write(&path, b"damaged engine")?;
        drop(trust);
        assert!(imported(root.path(), workspace.path(), context)?.stage(|| NOW, context).is_err());
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        assert_eq!(trust.updater_selection(context)?, before);
        assert_eq!(std::fs::read(path)?, b"damaged engine");
        Ok(())
    })
}
