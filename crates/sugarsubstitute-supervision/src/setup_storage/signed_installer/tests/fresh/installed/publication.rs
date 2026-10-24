// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! First-launch publication releases setup custody and recovers exact owned interruption points.
use super::*;
use crate::setup_storage::entrypoint;

/// Complete production phases under separate reservations while retaining one execution ledger.
fn prepared(pipeline: &mut Pipeline) -> Result<(tempfile::TempDir, SignedCandidate), Failure> {
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
        let temporary = tempfile::tempdir()?;
        let runtime = runtime(temporary.path())?;
        let candidate = import(temporary.path(), &context)?
            .prepare_installation(|| NOW, &context)?
            .complete(&runtime, || NOW, &context)?;
        Ok((temporary, candidate))
    })
}

#[test]
fn ready_publishes_both_entries_and_releases_setup_without_accepting_health() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (temporary, candidate) = prepared(&mut pipeline)?;
    let root = temporary.path().join("native");
    let selection = root.join(".selection-state/.substitute-selection.json");
    let before = std::fs::read(&selection)?;
    let completed = pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        Ok(candidate.authorize_installation(|| NOW, &context)?.publish(&context)?)
    })?;
    assert_eq!(completed.root(), root);
    assert_eq!(std::fs::read(&selection)?, before);
    assert_eq!(
        std::fs::read(root.join("sugarsubstitute-launcher.exe"))?,
        std::fs::read(completed.directory().join("sugarsubstitute-launcher.exe"))?
    );
    let inspected = root.clone();
    pipeline.run(LAUNCH_RESERVATION_BYTES, move |context| {
        assert_eq!(
            entrypoint::native::target(&inspected.join("SugarSubstitute.lnk"))?,
            inspected.join("sugarsubstitute-launcher.exe")
        );
        let summary = inspect_installation(&inspected, &context)?.ok_or("Settings absent")?;
        assert!(!summary.launch_entry_missing);
        assert!(!summary.executable_missing);
        let session = InstallationSession::maintain(&inspected, &context)?;
        drop(session);
        Ok(())
    })?;
    std::fs::remove_file(temporary.path().join("setup.exe"))?;
    let reopened = root.clone();
    let installed = pipeline.run(LAUNCH_RESERVATION_BYTES, move |context| {
        Ok(InstalledCandidate::reopen(&reopened, &context)?.ok_or("Durable choice missing")?)
    })?;
    assert_eq!(installed.directory(), completed.directory());
    let again = pipeline
        .run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| Ok(installed.publish(&context)?))?;
    assert_eq!(again.directory(), completed.directory());
    assert_eq!(std::fs::read(&selection)?, before);
    Ok(())
}

#[test]
fn unknown_root_executable_prevents_ready_without_overwriting_it() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (temporary, candidate) = prepared(&mut pipeline)?;
    let root = temporary.path().join("native");
    let executable = root.join("sugarsubstitute-launcher.exe");
    std::fs::write(&executable, b"user executable")?;
    pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        assert!(candidate.authorize_installation(|| NOW, &context)?.publish(&context).is_err());
        Ok(())
    })?;
    assert_eq!(std::fs::read(executable)?, b"user executable");
    assert!(!root.join("SugarSubstitute.lnk").exists());
    assert!(!root.join(".selection-state/.launcher-generation.json").exists());
    Ok(())
}

#[test]
fn edited_shortcut_is_preserved_and_missing_owned_entries_can_be_repaired() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (temporary, candidate) = prepared(&mut pipeline)?;
    let completed = pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        Ok(candidate.authorize_installation(|| NOW, &context)?.publish(&context)?)
    })?;
    let root = temporary.path().join("native");
    let link = root.join("SugarSubstitute.lnk");
    let original = std::fs::read(&link)?;
    std::fs::write(&link, b"user shortcut")?;
    let damaged = root.clone();
    pipeline.run(LAUNCH_RESERVATION_BYTES, move |context| {
        assert!(InstalledCandidate::reopen(&damaged, &context).is_err());
        Ok(())
    })?;
    assert_eq!(std::fs::read(&link)?, b"user shortcut");
    std::fs::write(&link, original)?;
    std::fs::remove_file(&link)?;
    std::fs::remove_file(root.join("sugarsubstitute-launcher.exe"))?;
    let installed = pipeline.run(LAUNCH_RESERVATION_BYTES, move |context| {
        Ok(InstalledCandidate::reopen(&root, &context)?.ok_or("Choice missing")?)
    })?;
    pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        installed.publish(&context)?;
        Ok(())
    })?;
    assert!(completed.root().join("sugarsubstitute-launcher.exe").is_file());
    assert!(link.is_file());
    Ok(())
}

#[test]
fn publication_does_not_take_application_run_custody() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let candidate = import(temporary.path(), context)?
            .prepare_installation(|| NOW, context)?
            .complete(&runtime(temporary.path())?, || NOW, context)?;
        let store = Arc::new(SelectionStore::open(&temporary.path().join("native"), context)?);
        let before = store.load(context)?;
        let _run = InstallationRun::acquire(store.clone(), context)?;
        let completed = candidate.authorize_installation(|| NOW, context)?.publish(context)?;
        assert!(completed.root().join("SugarSubstitute.lnk").is_file());
        assert_eq!(store.load(context)?, before);
        Ok(())
    })
}

#[test]
fn interrupted_entry_publication_resumes_from_each_owned_boundary() -> TestResult {
    for boundary in [
        ".selection-state/.launcher-generation.json",
        "sugarsubstitute-launcher.exe",
        ".selection-state/SugarSubstitute.lnk",
    ] {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().to_owned();
        worker::run_reserved_expiring(
            SIGNED_INSTALLER_RESERVATION_BYTES,
            move |context, clock| {
                let installed = import(&path, context)?
                    .prepare_installation(|| NOW, context)?
                    .complete(&runtime(&path)?, || NOW, context)?
                    .authorize_installation(|| NOW, context)?;
                let reached = path.join("native").join(boundary);
                clock.observe_checks(move || reached.is_file())?;
                assert!(installed.publish(context).is_err(), "Did not stop at {boundary}");
                Ok(())
            },
        )?;
        let root = temporary.path().join("native");
        worker::run_reserved(LAUNCH_RESERVATION_BYTES, move |context, _| {
            let installed = InstalledCandidate::reopen(&root, context)?.ok_or("Choice missing")?;
            let completed = installed.publish(context)?;
            assert_eq!(
                entrypoint::native::target(&root.join("SugarSubstitute.lnk"))?,
                root.join("sugarsubstitute-launcher.exe")
            );
            let state =
                SelectionStore::open(&root, context)?.load(context)?.ok_or("State absent")?;
            assert!(state.accepted().is_none());
            assert!(matches!(state.candidate(), Some(Candidate::Prepared { .. })));
            assert!(completed.directory().join("startup.local.toml").is_file());
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn explicit_preparation_rejects_historical_package_before_creating_candidate() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let imported = super::super::import(temporary.path(), context)?;
        let store = SelectionStore::open(&temporary.path().join("native"), context)?;
        let before = store.load(context)?;
        assert!(imported.prepare_installation(|| NOW, context).is_err());
        assert_eq!(store.load(context)?, before);
        assert!(!temporary.path().join("native/releases").exists());
        Ok(())
    })
}
