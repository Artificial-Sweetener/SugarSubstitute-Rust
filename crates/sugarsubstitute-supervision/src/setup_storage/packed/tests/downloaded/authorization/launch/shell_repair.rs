// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Repair actual Windows shortcuts from protected accepted selection and verified inert payloads.
use super::*;

#[test]
fn accepted_application_repairs_stale_and_missing_links_without_repository_or_cache() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (generation, workspace) = accept_fixture(&root, context)?;
        assert!(!workspace.exists());
        let link = root.path().join("SugarSubstitute.lnk");
        let old = std::fs::read(&link)?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let before = store.load(context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let InstallationLaunch::Application(application) = run.select_launch(None, context)? else {
            return Err("Accepted launch absent".into());
        };
        application.repair_launch_entry(context)?;
        let repaired = std::fs::read(&link)?;
        assert_ne!(repaired, old);
        assert_eq!(
            entrypoint::selected(&root, context)?.ok_or("Repaired entry absent")?.path(),
            root.path().join("releases").join(generation.id.as_str())
        );
        application.repair_launch_entry(context)?;
        assert_eq!(std::fs::read(&link)?, repaired);
        std::fs::remove_file(&link)?;
        application.repair_launch_entry(context)?;
        assert_eq!(std::fs::read(&link)?, repaired);
        assert_eq!(store.load(context)?, before);
        assert_eq!(std::fs::read(root.path().join("releases/installed/SugarSubstitute.lnk"))?, old);
        assert!(!root.path().join("external-comfy").exists());
        assert!(!root.path().join("python.exe").exists());
        Ok(())
    })
}

#[test]
fn accepted_repair_preserves_unknown_or_edited_shortcuts() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        for valid in [false, true] {
            let temporary = tempfile::tempdir()?;
            let root = Directory::open(temporary.path(), false)?;
            accept_fixture(&root, context)?;
            let link = root.path().join("SugarSubstitute.lnk");
            let mut edited = if valid { std::fs::read(&link)? } else { Vec::new() };
            edited.extend_from_slice(b"user customization");
            std::fs::write(&link, &edited)?;
            let store = Arc::new(SelectionStore::open(root.path(), context)?);
            let before = store.load(context)?;
            let mut run = InstallationRun::acquire(store.clone(), context)?;
            let InstallationLaunch::Application(application) = run.select_launch(None, context)?
            else {
                return Err("Accepted launch absent".into());
            };
            assert!(application.repair_launch_entry(context).is_err());
            assert_eq!(std::fs::read(&link)?, edited);
            assert_eq!(store.load(context)?, before);
            assert!(!application.directory().join("SugarSubstitute.lnk").exists());
        }
        Ok(())
    })
}

#[test]
fn repair_requires_mutation_exclusion_and_recovers_when_maintenance_finishes() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        accept_fixture(&root, context)?;
        let link = root.path().join("SugarSubstitute.lnk");
        let old = std::fs::read(&link)?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let mut run = InstallationRun::acquire(store, context)?;
        let InstallationLaunch::Application(application) = run.select_launch(None, context)? else {
            return Err("Accepted launch absent".into());
        };
        let maintenance = crate::setup_storage::transaction::admission::Admission::open(
            root.path(),
            false,
            context,
        )?;
        assert!(application.repair_launch_entry(context).is_err());
        assert_eq!(std::fs::read(&link)?, old);
        drop(maintenance);
        application.repair_launch_entry(context)?;
        assert_eq!(
            entrypoint::selected(&root, context)?.ok_or("Repair absent")?.path(),
            application.directory()
        );
        Ok(())
    })
}

#[test]
fn expired_repair_preserves_the_existing_shortcut_and_selection() -> TestResult {
    worker::run_reserved_expiring(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, clock| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        accept_fixture(&root, context)?;
        let link = root.path().join("SugarSubstitute.lnk");
        let old = std::fs::read(&link)?;
        let record = root.path().join(".selection-state/.substitute-selection.json");
        let before = std::fs::read(&record)?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let mut run = InstallationRun::acquire(store, context)?;
        let InstallationLaunch::Application(application) = run.select_launch(None, context)? else {
            return Err("Accepted launch absent".into());
        };
        clock.expire();
        assert!(application.repair_launch_entry(context).is_err());
        assert_eq!(std::fs::read(&link)?, old);
        assert_eq!(std::fs::read(&record)?, before);
        Ok(())
    })
}
