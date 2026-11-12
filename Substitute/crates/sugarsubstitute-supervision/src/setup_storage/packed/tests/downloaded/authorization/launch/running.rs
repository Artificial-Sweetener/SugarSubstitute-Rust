// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the actual elected generation directory to retained installation custody.
use super::*;
use crate::setup_storage::retain_running_generation;
mod image;

#[test]
fn elected_generation_retains_run_ownership_and_rejects_an_older_image_directory() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (generation, _) = accept_fixture(&root, context)?;
        assert!(
            retain_running_generation(&root.path().join("releases/installed"), None, context)
                .is_err()
        );
        let directory = root.path().join("releases").join(generation.id.as_str());
        let guard =
            retain_running_generation(&directory, None, context)?.ok_or("Run guard absent")?;
        assert_eq!(guard.generation(), &generation);
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        assert!(retain_running_generation(&directory, None, context).is_err());
        let before = store.load(context)?;
        let selected = before.as_ref().ok_or("Selection absent")?.installation().clone();
        assert_eq!(guard.installation_id(), &selected);
        let profile = sugarsubstitute_contracts::configuration::ProfileId::new("real".into())?;
        assert_eq!(
            crate::session_identity::session_host_identity(
                &root.path().join("user"),
                &profile,
                Some(&guard),
                context
            )?,
            selected
        );
        assert!(!root.path().join("user/sessions/real/source-host-identity.json").exists());
        drop(guard);
        let again =
            retain_running_generation(&directory, None, context)?.ok_or("Run did not retire")?;
        assert_eq!(again.generation(), &generation);
        assert_eq!(store.load(context)?, before);
        Ok(())
    })
}

#[test]
fn known_installation_cannot_bypass_run_ownership_by_losing_its_marker_or_selection() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        for marker in [false, true] {
            let temporary = tempfile::tempdir()?;
            let root = Directory::open(temporary.path(), false)?;
            let (generation, _) = accept_fixture(&root, context)?;
            let path = root.path().join(if marker {
                "state/installation.json"
            } else {
                "state/selection/.substitute-selection.json"
            });
            std::fs::remove_file(&path)?;
            assert!(
                retain_running_generation(
                    &root.path().join("releases").join(generation.id.as_str()),
                    None,
                    context
                )
                .is_err()
            );
            assert!(!path.exists());
        }
        Ok(())
    })
}

#[test]
fn source_and_earlier_native_launches_do_not_create_updater_selection() -> TestResult {
    worker::run_reserved(crate::setup_storage::LAUNCH_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source-build");
        assert!(retain_running_generation(&source, None, context)?.is_none());
        assert!(!source.exists());
        let root = Directory::open(temporary.path(), false)?;
        let older = fixture::installed(&root, context)?;
        assert!(retain_running_generation(&older, None, context)?.is_none());
        assert!(!root.path().join("state/selection").exists());
        Ok(())
    })
}

#[test]
fn initialized_selection_without_accepted_health_cannot_admit_a_generation() -> TestResult {
    worker::run_reserved(crate::setup_storage::LAUNCH_RESERVATION_BYTES, |context, _| {
        let root = crate::setup_storage::test_worker::installation(77, context)?;
        let directory = Directory::open(&root.path().join("releases").join("1".repeat(32)), true)?;
        assert!(retain_running_generation(directory.path(), None, context).is_err());
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let _run = InstallationRun::acquire(store, context)?;
        Ok(())
    })
}

#[test]
fn lost_namespace_and_ownership_cannot_disguise_an_enabled_installation_as_an_older_one()
-> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (generation, _) = accept_fixture(&root, context)?;
        let source = root.path().join("state/selection");
        let displaced = root.path().join("displaced-selection");
        std::fs::rename(&source, &displaced)?;
        let image = root.path().join("releases").join(generation.id.as_str());
        assert!(retain_running_generation(&image, None, context).is_err());
        std::fs::remove_file(root.path().join("state/installation.json"))?;
        assert!(retain_running_generation(&image, None, context).is_err());
        assert!(!source.exists());
        assert!(displaced.join(".substitute-selection.json").exists());
        Ok(())
    })
}
