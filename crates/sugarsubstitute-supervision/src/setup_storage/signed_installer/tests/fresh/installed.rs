// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Durable explicit choices survive restart while automatic candidates retain live trust checks.
use super::*;
use crate::application_launch::{ApplicationLaunch, CreationOutcome};
use crate::setup_storage::test_worker::pipeline::Pipeline;

const AUTHORIZATION: &str = "installation-authorization-v1.record";
mod forwarding;
mod publication;
mod rescue;

/// Explicit setup fixtures advertise the real reader separately from historical application entry.
fn import(path: &Path, context: &JobContext) -> Result<SignedInstallation, Failure> {
    import_session(
        path,
        InstallationSession::create(choices(path), context)?,
        PreparationIntent::Automatic,
        context,
    )
}

/// Repeat the same producer capability bytes when resuming a completed explicit setup.
fn import_session(
    path: &Path,
    session: InstallationSession,
    intent: PreparationIntent,
    context: &JobContext,
) -> Result<SignedInstallation, Failure> {
    use sugarsubstitute_contracts::application_launch::{
        self, installation::InstallationActivationContract,
    };
    let activation = InstallationActivationContract::produced(
        sugarsubstitute_update::ENGINE_RELEASE,
        sugarsubstitute_update::ENGINE_PROTOCOL,
    )?;
    let package = test_package::distribution_with_installation(
        Some(application_launch::CURRENT_CONTRACT),
        Some(application_launch::CURRENT_LAUNCHER_CONTRACT),
        None,
        Some(&activation),
    )?;
    super::import_distribution(path, session, intent, package, context)
}

#[test]
fn historical_launcher_cannot_commit_a_durable_installation_choice() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        use sugarsubstitute_contracts::application_launch::{
            CURRENT_CONTRACT, CURRENT_LAUNCHER_CONTRACT,
        };
        for launcher in [None, Some(CURRENT_LAUNCHER_CONTRACT)] {
            let temporary = tempfile::tempdir()?;
            let path = temporary.path();
            let package =
                test_package::distribution_with_launcher(Some(CURRENT_CONTRACT), launcher)?;
            let session = InstallationSession::create(choices(path), context)?;
            let candidate = super::import_distribution(
                path,
                session,
                PreparationIntent::Automatic,
                package,
                context,
            )?
            .prepare(|| NOW, context)?
            .complete(&runtime(path)?, || NOW, context)?;
            let directory = candidate.directory().to_owned();
            let store = SelectionStore::open(&path.join("native"), context)?;
            let before = store.load(context)?;
            assert!(candidate.authorize_installation(|| NOW, context).is_err());
            assert!(!directory.join(AUTHORIZATION).exists());
            assert_eq!(store.load(context)?, before);
        }
        Ok(())
    })
}

#[test]
fn incompatible_bundled_engine_cannot_commit_durable_installation_authority() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        use sugarsubstitute_contracts::application_launch::{
            self, installation::InstallationActivationContract,
        };
        let temporary = tempfile::tempdir()?;
        let path = temporary.path();
        let activation = InstallationActivationContract::produced(
            sugarsubstitute_update::ENGINE_RELEASE,
            sugarsubstitute_update::ENGINE_PROTOCOL + 1,
        )?;
        let package = test_package::distribution_with_installation(
            Some(application_launch::CURRENT_CONTRACT),
            Some(application_launch::CURRENT_LAUNCHER_CONTRACT),
            None,
            Some(&activation),
        )?;
        let session = InstallationSession::create(choices(path), context)?;
        let candidate = super::import_distribution(
            path,
            session,
            PreparationIntent::Automatic,
            package,
            context,
        )?
        .prepare(|| NOW, context)?
        .complete(&runtime(path)?, || NOW, context)?;
        let directory = candidate.directory().to_owned();
        let store = SelectionStore::open(&path.join("native"), context)?;
        let before = store.load(context)?;
        assert!(candidate.authorize_installation(|| NOW, context).is_err());
        assert!(!directory.join(AUTHORIZATION).exists());
        assert_eq!(store.load(context)?, before);
        Ok(())
    })
}

/// Separate original import, authorization and reopening under the production memory ledger.
#[test]
fn explicit_installation_reopens_without_metadata_and_retains_real_trial_recovery() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (temporary, candidate) = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
        let temporary = tempfile::tempdir()?;
        let runtime = runtime(temporary.path())?;
        let prepared = import(temporary.path(), &context)?.prepare(|| NOW, &context)?;
        let candidate = prepared.complete(&runtime, || NOW, &context)?;
        Ok((temporary, candidate))
    })?;
    let directory = candidate.directory().to_owned();
    let installed = pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        Ok(candidate.authorize_installation(|| NOW, &context)?)
    })?;
    let root = temporary.path().join("native");
    let selection = root.join("state/selection/.substitute-selection.json");
    let before = std::fs::read(&selection)?;
    assert!(directory.join(AUTHORIZATION).is_file());
    drop(installed);
    // Startup must not need the original installer or the repository's cached discovery bytes.
    std::fs::remove_file(temporary.path().join("setup.exe"))?;
    let reopened_root = root.clone();
    let installed = pipeline.run(LAUNCH_RESERVATION_BYTES, move |context| {
        Ok(Arc::new(
            InstalledCandidate::reopen(&reopened_root, &context)?.ok_or("Authorization missing")?,
        ))
    })?;
    assert_eq!(installed.directory(), directory);
    assert_eq!(std::fs::read(&selection)?, before);
    let launch = ApplicationLaunch::installed(installed.clone());
    let pending = launch.clone();
    pipeline.run(SIGNED_TRANSFER_RESERVATION_BYTES, move |context| {
        let mut clock_reads = 0;
        pending.prepare_trial(
            || {
                clock_reads += 1;
                EXPIRES
            },
            &context,
        )?;
        assert_eq!(clock_reads, 0);
        Ok(())
    })?;
    let state = std::fs::read(&selection)?;
    let state = sugarsubstitute_contracts::installation::InstallationRecord::decode(&state)?;
    assert!(state.accepted().is_none());
    assert!(matches!(state.candidate(), Some(Candidate::Trial { .. })));
    let pending = launch.clone();
    pipeline.run(crate::application_launch::CREATION_RESERVATION_BYTES, move |context| {
        let cause = pending.launch(&context).err().ok_or("Inert fixture unexpectedly executed")?;
        assert_eq!(cause.operation(), "application_launch_create");
        Ok(())
    })?;
    pipeline.run(crate::application_launch::RECOVERY_RESERVATION_BYTES, move |context| {
        assert_eq!(launch.recover_and_retire(&context)?, CreationOutcome::NotCreated);
        Ok(())
    })?;
    drop(installed);
    let running_root = root.clone();
    let run = pipeline.run(SELECTION_PUBLICATION_RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(&running_root, &context)?);
        let state = store.load(&context)?.ok_or("Selection absent")?;
        assert!(state.accepted().is_none());
        assert!(matches!(state.candidate(), Some(Candidate::Prepared { .. })));
        Ok(InstallationRun::acquire(store, &context)?)
    })?;
    pipeline.run(LAUNCH_RESERVATION_BYTES, move |context| {
        assert!(InstalledCandidate::reopen(&root, &context)?.is_some());
        Ok(())
    })?;
    drop(run);
    Ok(())
}

#[test]
fn ordinary_preparation_does_not_grant_durable_installation_authority() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let candidate = import(temporary.path(), context)?.prepare(|| NOW, context)?.complete(
            &runtime(temporary.path())?,
            || NOW,
            context,
        )?;
        let directory = candidate.directory().to_owned();
        drop(candidate);
        assert!(!directory.join(AUTHORIZATION).exists());
        assert!(InstalledCandidate::reopen(&temporary.path().join("native"), context)?.is_none());
        Ok(())
    })
}

#[test]
fn expired_repository_evidence_cannot_commit_an_installation_choice() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let candidate = import(temporary.path(), context)?.prepare(|| NOW, context)?.complete(
            &runtime(temporary.path())?,
            || NOW,
            context,
        )?;
        let directory = candidate.directory().to_owned();
        let store = SelectionStore::open(&temporary.path().join("native"), context)?;
        let before = store.load(context)?;
        assert!(candidate.authorize_installation(|| EXPIRES, context).is_err());
        assert!(!directory.join(AUTHORIZATION).exists());
        assert_eq!(store.load(context)?, before);
        Ok(())
    })
}

#[test]
fn corrupt_foreign_and_future_installation_authorizations_are_preserved() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let installed = import(temporary.path(), context)?
            .prepare(|| NOW, context)?
            .complete(&runtime(temporary.path())?, || NOW, context)?
            .authorize_installation(|| NOW, context)?;
        let record = installed.directory().join(AUTHORIZATION);
        let original = std::fs::read(&record)?;
        drop(installed);
        let root = temporary.path().join("native");
        let selection = root.join("state/selection/.substitute-selection.json");
        let before = std::fs::read(&selection)?;
        for (field, value) in [
            ("schema_version", serde_json::json!(2)),
            ("installation_directory", serde_json::json!([0, 0, 0])),
            ("generation_directory", serde_json::json!([0, 0, 0])),
            ("minimum_updater_release", serde_json::json!(u64::MAX)),
            ("updater_protocol", serde_json::json!(u32::MAX)),
        ] {
            let mut changed: serde_json::Value = serde_json::from_slice(&original)?;
            changed[field] = value;
            let bytes = serde_json::to_vec(&changed)?;
            std::fs::write(&record, &bytes)?;
            assert!(InstalledCandidate::reopen(&root, context).is_err(), "accepted {field}");
            assert_eq!(std::fs::read(&record)?, bytes);
            assert_eq!(std::fs::read(&selection)?, before);
        }
        std::fs::write(&record, b"truncated")?;
        assert!(InstalledCandidate::reopen(&root, context).is_err());
        assert_eq!(std::fs::read(record)?, b"truncated");
        Ok(())
    })
}

#[test]
fn durable_authorization_does_not_bypass_runtime_verification() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let installed = import(temporary.path(), context)?
            .prepare(|| NOW, context)?
            .complete(&runtime(temporary.path())?, || NOW, context)?
            .authorize_installation(|| NOW, context)?;
        let image = installed.directory().join("sugarsubstitute.exe");
        drop(installed);
        std::fs::write(&image, b"damaged runtime")?;
        assert!(InstalledCandidate::reopen(&temporary.path().join("native"), context).is_err());
        assert_eq!(std::fs::read(image)?, b"damaged runtime");
        Ok(())
    })
}

#[test]
fn durable_choice_does_not_take_or_bypass_live_application_run_custody() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let candidate = import(temporary.path(), context)?.prepare(|| NOW, context)?.complete(
            &runtime(temporary.path())?,
            || NOW,
            context,
        )?;
        let store = Arc::new(SelectionStore::open(&temporary.path().join("native"), context)?);
        let run = InstallationRun::acquire(store.clone(), context)?;
        let installed = candidate.authorize_installation(|| NOW, context)?;
        let before = store.load(context)?;
        let mut recovery = None;
        assert!(installed.transfer(context, &mut recovery).is_err());
        assert!(recovery.is_none());
        assert_eq!(store.load(context)?, before);
        drop(run);
        let transfer = installed.transfer(context, &mut recovery)?;
        assert_eq!(transfer.directory(), installed.directory());
        Ok(())
    })
}

#[test]
fn repeated_explicit_authorization_preserves_the_exact_durable_choice() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let runtime = runtime(temporary.path())?;
        let installed = import(temporary.path(), context)?
            .prepare(|| NOW, context)?
            .complete(&runtime, || NOW, context)?
            .authorize_installation(|| NOW, context)?;
        let directory = installed.directory().to_owned();
        let original = std::fs::read(directory.join(AUTHORIZATION))?;
        drop(installed);
        let session = InstallationSession::maintain(&temporary.path().join("native"), context)?;
        let installed =
            import_session(temporary.path(), session, PreparationIntent::Automatic, context)?
                .prepare(|| NOW, context)?
                .complete(&runtime, || NOW, context)?
                .authorize_installation(|| NOW, context)?;
        assert_eq!(installed.directory(), directory);
        assert_eq!(std::fs::read(directory.join(AUTHORIZATION))?, original);
        Ok(())
    })
}
