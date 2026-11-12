// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Accepted launch admission uses actual protected storage and pinned inert payloads, without cache.
use super::*;
use crate::setup_storage::{InstallationLaunch, InstallationRun, SelectionStore};
use sugarsubstitute_contracts::installation::{
    Generation, GenerationId, PackageDigest, PreparationIntent,
};
mod compatibility;
mod inheritance;
mod maintenance;
mod recovery;
mod reservation;
mod running;
mod shell_repair;
mod stable;
mod stable_entry;

/// Simulate the host's health decision at the storage boundary; fixture payloads are never executed.
pub(super) fn accept_fixture(
    directory: &Directory,
    context: &JobContext,
) -> Result<(Generation, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    accept_distribution(directory, test_package::distribution()?, context)
}

/// Exercise the real package and trial storage owners with the requested inert distribution.
fn accept_distribution(
    directory: &Directory,
    distribution: (Vec<u8>, Vec<u8>),
    context: &JobContext,
) -> Result<(Generation, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    fixture::installed(directory, context)?;
    let workspace = tempfile::tempdir()?;
    let (manifest, compressed) = distribution;
    let archive = test_package::seal(&manifest, &compressed);
    let signed = SignedPackage::new(&archive, 1)?;
    let trust = signed.initialize(directory.path(), context)?;
    let target = trust
        .offer(signed.metadata.metadata(), &signed.request, &signed.descriptor, NOW, context)?
        .into_package()?;
    let bundle = Arc::new(VerifiedBundle::from_download(
        receive(&archive, directory.path(), target, context)?,
        &crate::setup_storage::cache::RuntimeCache::open(
            workspace.path(),
            crate::setup_storage::cache::CachePolicy::default(),
            context,
        )?,
        context,
    )?);
    let preparation =
        crate::setup_storage::InstallationSession::maintain(directory.path(), context)?
            .prepare(bundle, context)?;
    let candidate = preparation.prepare_update(
        preparation.maintenance_runtime().ok_or("Runtime absent")?,
        &signed.authorization(&trust),
        || NOW,
        context,
    )?;
    let generation = candidate.generation().clone();
    let store = Arc::new(SelectionStore::open(directory.path(), context)?);
    let mut owner = InstallationRun::acquire(store, context)?;
    let admitted =
        candidate.begin_trial(&mut owner, &signed.authorization(&trust), || NOW, context)?;
    owner.accept(admitted.trial(), context)?;
    Ok((generation, workspace.path().to_owned()))
}

#[test]
fn accepted_launch_needs_no_extracted_cache_or_repository_and_retains_native_custody() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let (generation, workspace) = accept_fixture(&directory, context)?;
        assert!(!workspace.exists());
        std::fs::write(
            directory.path().join("state/update-trust/checkpoint"),
            b"repository needs repair",
        )?;
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let mut owner = InstallationRun::acquire(store.clone(), context)?;
        let InstallationLaunch::Application(application) = owner.select_launch(None, context)?
        else {
            return Err("Accepted generation was not selected".into());
        };
        assert_eq!(application.generation(), &generation);
        assert_eq!(
            application.directory(),
            directory.path().join("releases").join(generation.id.as_str())
        );
        assert_eq!(application.executable(), application.directory().join("sugarsubstitute.exe"));
        assert!(owner.select_launch(None, context).is_err());
        assert!(std::fs::write(application.executable(), b"changed").is_err());
        assert!(std::fs::rename(application.directory(), directory.path().join("moved")).is_err());
        drop(owner);
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        let before = store.load(context)?.ok_or("Selection absent")?;
        let pending = before.prepare(
            Generation {
                id: GenerationId::new("2".repeat(32))?,
                package: PackageDigest::new("c".repeat(64))?,
                release: sugarsubstitute_contracts::installation::SignedRelease::new(
                    "1.0.0".to_owned(),
                    1,
                )?,
            },
            PreparationIntent::Automatic,
        )?;
        store.compare_exchange(&before, &pending, context)?;
        assert_eq!(store.load(context)?.ok_or("Selection absent")?.accepted(), Some(&generation));
        drop(application);
        let mut next = InstallationRun::acquire(store, context)?;
        assert!(matches!(next.select_launch(None, context)?, InstallationLaunch::Application(_)));
        Ok(())
    })
}

#[test]
fn changed_inventory_or_application_bytes_cannot_supply_an_accepted_launch() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        for leaf in ["application-package.json", "sugarsubstitute.exe"] {
            let temporary = tempfile::tempdir()?;
            let directory = Directory::open(temporary.path(), false)?;
            let (generation, _) = accept_fixture(&directory, context)?;
            let path = directory.path().join("releases").join(generation.id.as_str()).join(leaf);
            let mut edited = std::fs::read(&path)?;
            edited[0] ^= 1;
            std::fs::write(&path, &edited)?;
            let store = Arc::new(SelectionStore::open(directory.path(), context)?);
            let before = store.load(context)?;
            let mut owner = InstallationRun::acquire(store.clone(), context)?;
            assert!(owner.select_launch(None, context).is_err());
            assert_eq!(store.load(context)?, before);
            assert_eq!(std::fs::read(path)?, edited);
        }
        Ok(())
    })
}

#[test]
fn released_run_lock_recovers_previous_acceptance_without_accepting_the_interrupted_trial()
-> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let (healthy, _) = accept_fixture(&directory, context)?;
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let before = store.load(context)?.ok_or("Selection absent")?;
        let generation = Generation {
            id: GenerationId::new("2".repeat(32))?,
            package: PackageDigest::new("c".repeat(64))?,
            release: sugarsubstitute_contracts::installation::SignedRelease::new(
                "1.0.0".to_owned(),
                1,
            )?,
        };
        let staging = before.prepare(generation.clone(), PreparationIntent::Automatic)?;
        store.compare_exchange(&before, &staging, context)?;
        let prepared = staging.prepared(&generation)?;
        store.compare_exchange(&staging, &prepared, context)?;
        let mut owner = InstallationRun::acquire(store.clone(), context)?;
        let trial = owner.begin_trial(&prepared, context)?;
        let expected = trial.attempt().clone();
        drop(trial);
        drop(owner);
        let mut reopened = InstallationRun::acquire(store.clone(), context)?;
        let InstallationLaunch::Application(actual) = reopened.select_launch(None, context)? else {
            return Err("Accepted generation was not recovered".into());
        };
        assert_eq!(actual.generation(), &healthy);
        let recovered = store.load(context)?.ok_or("Recovered selection absent")?;
        assert_eq!(recovered.rejected(), &[expected.generation.package]);
        assert!(recovered.candidate().is_none());
        Ok(())
    })
}

#[test]
fn empty_selection_requests_installation_without_inventing_an_executable() -> TestResult {
    worker::run_reserved(crate::setup_storage::LAUNCH_RESERVATION_BYTES, |context, _| {
        let root = crate::setup_storage::test_worker::installation(55, context)?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let mut owner = InstallationRun::acquire(store, context)?;
        assert!(matches!(
            owner.select_launch(None, context)?,
            InstallationLaunch::NeedsInstallation
        ));
        assert!(!root.path().join("releases").exists());
        Ok(())
    })
}
