// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticated preparation refreshes launcher recovery authority while trial admission rechecks it.
use super::*;

/// Prepare the next authenticated offer against the same actual installation and trust store.
fn next_package(
    root: &Directory,
    workspace: &Path,
    context: &JobContext,
) -> Result<
    (PreparedInstallation, SignedPackage, TrustStore),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let (manifest, compressed) = test_package::distribution_with_launcher(
        Some(CURRENT_CONTRACT),
        Some(CURRENT_LAUNCHER_CONTRACT),
    )?;
    let mut value: serde_json::Value = serde_json::from_slice(&manifest)?;
    value["release_id"] = "next-release".into();
    let archive = test_package::seal(&serde_json::to_vec(&value)?, &compressed);
    let signed = SignedPackage::new(&archive, 2)?;
    let trust = TrustStore::open(&root.path, [92; 32], context)?;
    let target = trust
        .offer(signed.metadata.metadata(), &signed.request, &signed.descriptor, NOW, context)?
        .into_package()?;
    let bundle = Arc::new(VerifiedBundle::from_download(
        receive(&archive, &root.path, target, context)?,
        workspace,
        context,
    )?);
    let prepared = crate::setup_storage::InstallationSession::maintain(&root.path, context)?
        .prepare(bundle, context)?;
    Ok((prepared, signed, trust))
}

#[test]
fn preparation_and_trial_admission_recheck_the_installed_launcher() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let workspace = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let application = prepare(&root, context)?;
        application.repair_launch_entry(context)?;
        drop(application);
        let ownership = root.path.join(OWNERSHIP);
        let original = std::fs::read(&ownership)?;
        let mut changed: serde_json::Value = serde_json::from_slice(&original)?;
        changed["contract"] =
            r#"{"schema_version":1,"installation_schemas":[1],"installed_launch_protocol":2}"#
                .into();
        let incompatible = serde_json::to_vec(&changed)?;
        let (prepared, signed, trust) = next_package(&root, workspace.path(), context)?;
        let store = Arc::new(SelectionStore::open(&root.path, context)?);
        let before = store.load(context)?;
        std::fs::write(&ownership, &incompatible)?;
        assert!(
            prepared
                .prepare_update(
                    prepared.maintenance_runtime().ok_or("Runtime absent")?,
                    &signed.authorization(&trust),
                    || NOW,
                    context
                )
                .is_err()
        );
        assert_eq!(store.load(context)?, before);
        std::fs::write(&ownership, &original)?;
        let candidate = prepared.prepare_update(
            prepared.maintenance_runtime().ok_or("Runtime absent")?,
            &signed.authorization(&trust),
            || NOW,
            context,
        )?;
        let before_trial = store.load(context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        std::fs::write(&ownership, &incompatible)?;
        assert!(
            candidate
                .begin_trial(&mut run, &signed.authorization(&trust), || NOW, context)
                .is_err()
        );
        assert_eq!(store.load(context)?, before_trial);
        Ok(())
    })
}

#[test]
fn prepared_application_refreshes_launcher_source_without_replacing_identical_bytes() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let workspace = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let application = prepare(&root, context)?;
        application.repair_launch_entry(context)?;
        let first = application.generation().clone();
        drop(application);
        let ownership = std::fs::read(root.path.join(OWNERSHIP))?;
        let identity = crate::setup_storage::files::file_identity(&root.read(LAUNCHER)?)?;
        let (prepared, signed, trust) = next_package(&root, workspace.path(), context)?;
        let candidate = prepared.prepare_update(
            prepared.maintenance_runtime().ok_or("Runtime absent")?,
            &signed.authorization(&trust),
            || NOW,
            context,
        )?;
        let store = Arc::new(SelectionStore::open(&root.path, context)?);
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let admitted =
            candidate.begin_trial(&mut run, &signed.authorization(&trust), || NOW, context)?;
        let accepted = run.accept(admitted.trial(), context)?;
        assert_ne!(accepted.accepted(), Some(&first));
        drop(admitted);
        drop(prepared);
        drop(run);
        let mut run = InstallationRun::acquire(store, context)?;
        let InstallationLaunch::Application(next) = run.select_launch(None, context)? else {
            return Err("Next accepted generation absent".into());
        };
        let installed = next.repair_launch_entry(context)?;
        assert_eq!(installed.executable, root.path.join(LAUNCHER));
        let next_ownership = std::fs::read(root.path.join(OWNERSHIP))?;
        assert_ne!(next_ownership, ownership);
        let next_record: serde_json::Value = serde_json::from_slice(&next_ownership)?;
        assert_eq!(next_record["source"]["id"], next.generation().id.as_str());
        assert_eq!(crate::setup_storage::files::file_identity(&root.read(LAUNCHER)?)?, identity);
        assert!(root.path.join("releases").join(first.id.as_str()).join(LAUNCHER).exists());
        Ok(())
    })
}

#[test]
fn signed_preparation_rescues_missing_launcher_with_obsolete_pending_source() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let workspace = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let application = prepare(&root, context)?;
        application.repair_launch_entry(context)?;
        let accepted = application.generation().clone();
        let source = application.directory().to_path_buf();
        drop(application);
        let original: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.path.join(OWNERSHIP))?)?;
        let mut obsolete = original.clone();
        obsolete["source"]["id"] = "b".repeat(32).into();
        obsolete["source"]["package"] = "b".repeat(64).into();
        let journal = serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "installation_directory": root.identity()?,
            "previous": original,
            "next": obsolete,
        }))?;
        let state = Directory::open(&root.path.join(".selection-state"), false)?;
        crate::setup_storage::protection::with(context, |protection| {
            crate::setup_storage::publication::write_immutable(
                &state,
                ".launcher-transition-v1.json",
                &journal,
                protection,
                context,
            )
        })?;
        std::fs::remove_file(root.path.join(LAUNCHER))?;
        std::fs::remove_file(source.join(LAUNCHER))?;
        std::fs::write(root.path.join("user-document.txt"), b"preserve user work")?;
        let link = std::fs::read(root.path.join(LINK))?;
        let (prepared, signed, trust) = next_package(&root, workspace.path(), context)?;
        let store = Arc::new(SelectionStore::open(&root.path, context)?);
        // Application run custody remains available to the live app during signed repair.
        let _run = InstallationRun::acquire(store.clone(), context)?;
        let candidate = prepared.prepare_update(
            prepared.maintenance_runtime().ok_or("Runtime absent")?,
            &signed.authorization(&trust),
            || NOW,
            context,
        )?;
        assert_eq!(store.load(context)?.ok_or("Selection absent")?.accepted(), Some(&accepted));
        let owner: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.path.join(OWNERSHIP))?)?;
        assert_eq!(owner["source"]["id"], candidate.generation().id.as_str());
        assert_eq!(
            std::fs::read(root.path.join(LAUNCHER))?,
            std::fs::read(candidate.directory().join(LAUNCHER))?
        );
        assert_eq!(std::fs::read(root.path.join(LINK))?, link);
        assert_eq!(entrypoint::native::target(&root.path.join(LINK))?, root.path.join(LAUNCHER));
        assert_eq!(std::fs::read(root.path.join("user-document.txt"))?, b"preserve user work");
        assert!(!state.path.join(".launcher-transition-v1.json").exists());
        Ok(())
    })
}
