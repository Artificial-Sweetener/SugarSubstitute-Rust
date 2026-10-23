// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! App updates retain and recheck the independently installed launcher.
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
        &crate::setup_storage::cache::RuntimeCache::open(
            workspace,
            crate::setup_storage::cache::CachePolicy::default(),
            context,
        )?,
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
fn accepting_another_application_retains_original_launcher_ownership_and_image() -> TestResult {
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
        assert_eq!(std::fs::read(root.path.join(OWNERSHIP))?, ownership);
        assert_eq!(crate::setup_storage::files::file_identity(&root.read(LAUNCHER)?)?, identity);
        assert!(root.path.join("releases").join(first.id.as_str()).join(LAUNCHER).exists());
        Ok(())
    })
}
