// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real protected selection receives prepared packages without implicit health or launch changes.
use super::*;
use crate::setup_storage::{InstallationRun, SelectionStore};
use sugarsubstitute_contracts::installation::{
    Candidate, Generation, GenerationId, InstallationRecord, PackageDigest, PreparationIntent,
};

/// Seed a prior domain observation at the storage boundary; this fixture launches no process.
fn rejected(
    case: &Case<'_>,
    store: &Arc<SelectionStore>,
) -> Result<InstallationRecord, Box<dyn std::error::Error + Send + Sync>> {
    let empty = store.load(case.context)?.ok_or("Selection absent")?;
    let generation = Generation {
        id: GenerationId::new("1".repeat(32))?,
        package: PackageDigest::new(format!(
            "{:x}",
            Sha256::digest(test_package::distribution()?.0)
        ))?,
    };
    let staging = empty.prepare(generation.clone(), PreparationIntent::Automatic)?;
    store.compare_exchange(&empty, &staging, case.context)?;
    let prepared = staging.prepared(&generation)?;
    store.compare_exchange(&staging, &prepared, case.context)?;
    let mut owner = InstallationRun::acquire(Arc::clone(store), case.context)?;
    let attempt = owner.begin_trial(&prepared, case.context)?;
    Ok(owner.reject(&attempt, case.context)?)
}

#[test]
fn preparation_preserves_accepted_generation_and_coexists_with_installation_run() -> TestResult {
    run_accepted(|case| {
        let store = Arc::new(SelectionStore::open(case.directory.path(), case.context)?);
        let before = store.load(case.context)?.ok_or("Selection absent")?;
        let mut running = InstallationRun::acquire(Arc::clone(&store), case.context)?;
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        let reopened = SelectionStore::open(case.directory.path(), case.context)?;
        let after = reopened.load(case.context)?.ok_or("Selection absent")?;
        assert_eq!(after.accepted(), before.accepted());
        assert_eq!(after.retained(), before.retained());
        assert_eq!(after.rejected(), before.rejected());
        assert_eq!(after.revision(), before.revision() + 2);
        assert!(matches!(after.candidate(), Some(Candidate::Prepared { generation })
            if generation == candidate.generation()));
        case.preserved()?;
        let attempt = running.begin_trial(&after, case.context)?;
        assert_eq!(&attempt.attempt().generation, candidate.generation());
        assert_eq!(store.load(case.context)?.ok_or("Trial absent")?.accepted(), before.accepted());
        case.preserved()
    })
}

#[test]
fn expired_final_check_retains_staging_and_never_admits_trial() -> TestResult {
    for expired_check in [3, 4] {
        run_accepted(move |case| {
            let store = Arc::new(SelectionStore::open(case.directory.path(), case.context)?);
            let before = store.load(case.context)?.ok_or("Selection absent")?;
            let mut checks = 0;
            assert!(
                case.prepared
                    .prepare_update(
                        case.runtime,
                        &case.signed.authorization(case.trust),
                        || {
                            checks += 1;
                            if checks == expired_check { EXPIRES } else { NOW }
                        },
                        case.context,
                    )
                    .is_err()
            );
            assert_eq!(checks, expired_check);
            let after = store.load(case.context)?.ok_or("Selection absent")?;
            assert_eq!(after.accepted(), before.accepted());
            assert_eq!(after.revision(), before.revision() + 1);
            let Some(Candidate::Staging { generation }) = after.candidate() else {
                return Err("Expired candidate escaped staging".into());
            };
            assert_eq!(
                case.directory
                    .path()
                    .join("releases")
                    .join(generation.id.as_str())
                    .join("application-update-v1.record")
                    .exists(),
                expired_check == 4
            );
            let mut owner = InstallationRun::acquire(store, case.context)?;
            assert!(owner.begin_trial(&after, case.context).is_err());
            case.preserved()
        })?;
    }
    Ok(())
}

#[test]
fn automatic_quarantine_requires_explicit_repair_before_candidate_publication() -> TestResult {
    run(1, |case| {
        let store = Arc::new(SelectionStore::open(case.directory.path(), case.context)?);
        let before = rejected(&case, &store)?;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &case.signed.authorization(case.trust),
                    || NOW,
                    case.context,
                )
                .is_err()
        );
        assert_eq!(store.load(case.context)?, Some(before.clone()));
        case.preserved()?;
        let mut repair = SignedPackage::new(case.archive, 1)?;
        repair.request.intent = PreparationIntent::Repair;
        let authorization = repair.authorization(case.trust);
        let candidate =
            case.prepared.prepare_update(case.runtime, &authorization, || NOW, case.context)?;
        let after = store.load(case.context)?.ok_or("Selection absent")?;
        assert!(matches!(after.candidate(), Some(Candidate::Prepared { generation })
            if generation == candidate.generation()));
        assert_eq!(after.rejected(), before.rejected());
        case.preserved()
    })
}

#[test]
fn selection_publication_failure_preserves_old_record_and_allows_clean_retry() -> TestResult {
    run(1, |case| {
        let store = SelectionStore::open(case.directory.path(), case.context)?;
        let before = store.load(case.context)?;
        let protected = Directory::open(&case.directory.path().join(".selection-state"), false)?;
        let pinned = protected.read(".substitute-selection.json")?;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &case.signed.authorization(case.trust),
                    || NOW,
                    case.context,
                )
                .is_err()
        );
        assert_eq!(store.load(case.context)?, before);
        case.preserved()?;
        drop(pinned);
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        let after = store.load(case.context)?.ok_or("Selection absent")?;
        assert!(matches!(after.candidate(), Some(Candidate::Prepared { generation })
            if generation == candidate.generation()));
        case.preserved()
    })
}

#[test]
fn altered_destination_rejects_preparation_and_verified_candidate_pins_exact_payloads() -> TestResult
{
    run(1, |case| {
        let store = SelectionStore::open(case.directory.path(), case.context)?;
        let before = store.load(case.context)?;
        let directory = std::fs::read_dir(case.directory.path().join("releases"))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<_>>>()?
            .into_iter()
            .find(|path| path != case.original)
            .ok_or("Candidate absent")?;
        let executable = directory.join("sugarsubstitute.exe");
        let original = std::fs::read(&executable)?;
        let mut altered = original.clone();
        altered[0] ^= 1;
        std::fs::write(&executable, &altered)?;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &case.signed.authorization(case.trust),
                    || NOW,
                    case.context,
                )
                .is_err()
        );
        let interrupted = store.load(case.context)?.ok_or("Staging absent")?;
        assert!(matches!(interrupted.candidate(), Some(Candidate::Staging { .. })));
        assert_eq!(interrupted.accepted(), before.as_ref().and_then(InstallationRecord::accepted));
        assert_eq!(std::fs::read(&executable)?, altered);
        case.preserved()?;
        std::fs::write(&executable, &original)?;
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        assert_eq!(candidate.directory(), directory);
        assert!(std::fs::write(&executable, &altered).is_err());
        assert!(std::fs::remove_file(&executable).is_err());
        assert!(std::fs::rename(&directory, case.directory.path().join("moved")).is_err());
        let prepared = store.load(case.context)?;
        let generation = candidate.generation().clone();
        drop(candidate);
        let repeated = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        assert_eq!(repeated.generation(), &generation);
        assert_eq!(store.load(case.context)?, prepared);
        assert_eq!(std::fs::read(executable)?, original);
        case.preserved()
    })
}
