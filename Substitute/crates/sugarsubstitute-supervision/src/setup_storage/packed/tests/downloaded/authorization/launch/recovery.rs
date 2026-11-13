// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover launch selection independently of payload retirement after lost trial ownership.
use super::*;
use crate::setup_storage::{OwnedTrial, ReservedLaunch, reserve_installed_launch};

/// Admit an inert second generation without reporting process creation or health.
fn pending(
    store: &Arc<SelectionStore>,
    context: &JobContext,
) -> Result<(InstallationRun, OwnedTrial), Box<dyn std::error::Error + Send + Sync>> {
    let before = store.load(context)?.ok_or("Selection absent")?;
    let generation = Generation {
        id: GenerationId::new("2".repeat(32))?,
        package: PackageDigest::new("c".repeat(64))?,
        release: sugarsubstitute_contracts::installation::SignedRelease::new(
            "1.0.0".to_owned(),
            1,
        )?,
    };
    let staging = before.prepare(generation.clone(), PreparationIntent::Automatic, PREPARED_AT)?;
    store.compare_exchange(&before, &staging, context)?;
    let prepared = staging.prepared(&generation)?;
    store.compare_exchange(&staging, &prepared, context)?;
    let mut run = InstallationRun::acquire(store.clone(), context)?;
    let trial = run.begin_trial(&prepared, context)?;
    Ok((run, trial))
}

#[test]
fn interrupted_trial_restores_accepted_launch_without_retiring_candidate_files() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let (healthy, _) = accept_fixture(&directory, context)?;
        let store = Arc::new(SelectionStore::open(temporary.path(), context)?);
        let (run, trial) = pending(&store, context)?;
        let failed = trial.attempt().generation.clone();
        let candidate = temporary.path().join("releases").join(failed.id.as_str());
        std::fs::create_dir(&candidate)?;
        let unknown = candidate.join("preserve.bin");
        std::fs::write(&unknown, b"retained candidate data")?;
        let user = temporary.path().join("user/desktop-data").join("user-project.bin");
        std::fs::write(&user, b"authored data")?;
        drop(trial);
        drop(run);
        let ReservedLaunch::Application(application) =
            reserve_installed_launch(temporary.path(), context)?
        else {
            return Err("Abandoned trial still prevents accepted launch".into());
        };
        assert_eq!(application.generation(), &healthy);
        let recovered = store.load(context)?.ok_or("Recovered selection absent")?;
        assert_eq!(recovered.accepted(), Some(&healthy));
        assert!(recovered.strikes().is_empty());
        assert_eq!(
            recovered.candidate(),
            Some(&sugarsubstitute_contracts::installation::Candidate::Prepared {
                generation: failed.clone()
            })
        );
        assert_eq!(std::fs::read(unknown)?, b"retained candidate data");
        assert_eq!(std::fs::read(user)?, b"authored data");
        drop(application);
        assert!(matches!(
            reserve_installed_launch(temporary.path(), context)?,
            ReservedLaunch::Application(_)
        ));
        assert_eq!(store.load(context)?, Some(recovered));
        Ok(())
    })
}

#[test]
fn interrupted_trial_recovery_requires_every_original_run_holder_to_release() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        accept_fixture(&directory, context)?;
        let store = Arc::new(SelectionStore::open(temporary.path(), context)?);
        let (run, trial) = pending(&store, context)?;
        let before = store.load(context)?;
        drop(run);
        assert!(reserve_installed_launch(temporary.path(), context).is_err());
        assert_eq!(store.load(context)?, before);
        drop(trial);
        assert!(matches!(
            reserve_installed_launch(temporary.path(), context)?,
            ReservedLaunch::Application(_)
        ));
        Ok(())
    })
}
