// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real Windows file exclusion and publication, executed through the production job owner.
use super::*;
use crate::setup_storage::test_worker as worker;
use std::error::Error;
use sugarsubstitute_contracts::installation::{
    Generation, GenerationId, InstallationId, PackageDigest, PreparationIntent, Rejection,
};
mod locks;
pub(super) type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

/// The fixed UTC Unix time these records are prepared and rejected at. Each test passes this
/// time explicitly, so no outcome depends on the real clock.
pub(super) const PREPARED_AT: u64 = 1_800_000_000;

pub(super) fn root() -> Result<tempfile::TempDir, Box<dyn Error + Send + Sync>> {
    let root = tempfile::tempdir()?;
    std::fs::create_dir(root.path().join("state"))?;
    std::fs::write(
        root.path().join("state/installation.json"),
        br#"{"schema_version":1,"product":"SugarSubstitute"}"#,
    )?;
    Ok(root)
}
pub(super) fn initial() -> Result<InstallationRecord, Rejection> {
    Ok(InstallationRecord::empty(InstallationId::new("1".repeat(32))?))
}
pub(super) fn generation(number: u64) -> Result<Generation, Rejection> {
    Ok(Generation {
        id: GenerationId::new(format!("{number:032x}"))?,
        package: PackageDigest::new(format!("{number:064x}"))?,
        release: sugarsubstitute_contracts::installation::SignedRelease::new(
            format!("1.0.{number}"),
            number + 1,
        )?,
    })
}

#[test]
fn competing_writers_compare_exact_state_and_never_overwrite_a_new_selection() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let initial = initial()?;
        let first = SelectionStore::initialize(root.path(), &initial, context)?;
        let second = SelectionStore::open(root.path(), context)?;
        assert!(matches!(
            SelectionStore::initialize(root.path(), &initial, context),
            Err(SelectionFailure::Changed)
        ));
        let a = initial.prepare(generation(1)?, PreparationIntent::Automatic, PREPARED_AT)?;
        let b = initial.prepare(generation(2)?, PreparationIntent::Automatic, PREPARED_AT)?;
        first.compare_exchange(&initial, &a, context)?;
        assert!(matches!(
            second.compare_exchange(&initial, &b, context),
            Err(SelectionFailure::Changed)
        ));
        assert_eq!(second.load(context)?, Some(a));
        Ok(())
    })
}

#[test]
fn publication_reloads_complete_records_and_ignores_interrupted_staging_files() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let initial = initial()?;
        let candidate = generation(1)?;
        let staging =
            initial.prepare(candidate.clone(), PreparationIntent::Automatic, PREPARED_AT)?;
        {
            let store = SelectionStore::initialize(root.path(), &initial, context)?;
            std::fs::write(root.path().join(".selection-stage-interrupted"), b"{partial")?;
            assert_eq!(store.load(context)?, Some(initial.clone()));
            store.compare_exchange(&initial, &staging, context)?;
        }
        let store = SelectionStore::open(root.path(), context)?;
        assert_eq!(store.load(context)?, Some(staging.clone()));
        let prepared = staging.prepared(&candidate)?;
        store.compare_exchange(&staging, &prepared, context)?;
        assert_eq!(store.load(context)?, Some(prepared));
        assert_eq!(std::fs::read(root.path().join(".selection-stage-interrupted"))?, b"{partial");
        Ok(())
    })
}

#[test]
fn failed_replacement_keeps_original_selection_and_can_be_retried() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let initial = initial()?;
        let store = SelectionStore::initialize(root.path(), &initial, context)?;
        let replacement =
            initial.prepare(generation(1)?, PreparationIntent::Automatic, PREPARED_AT)?;
        let pin = store.root.read(RECORD)?;
        assert!(matches!(
            store.compare_exchange(&initial, &replacement, context),
            Err(SelectionFailure::Storage(_))
        ));
        assert_eq!(store.load(context)?, Some(initial.clone()));
        drop(pin);
        store.compare_exchange(&initial, &replacement, context)?;
        assert_eq!(store.load(context)?, Some(replacement));
        Ok(())
    })
}

/// The lock file persists between owners; only a live holder ever makes a waiter wait.
#[test]
fn a_leftover_lock_file_is_not_a_lockout() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let first = SelectionStore::initialize(root.path(), &initial()?, context)?;
        drop(first);
        assert!(root.path().join("state").join(DIRECTORY).join(LOCK).exists());
        let second = SelectionStore::open(root.path(), context)?;
        assert_eq!(second.load(context)?, Some(initial()?));
        Ok(())
    })
}

#[test]
fn ownership_marker_cannot_be_replaced_or_modified_while_store_is_open() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = SelectionStore::initialize(root.path(), &initial()?, context)?;
        let marker = root.path().join("state/installation.json");
        let original = std::fs::read(&marker)?;
        assert!(std::fs::write(&marker, b"changed").is_err());
        assert!(std::fs::remove_file(&marker).is_err());
        assert_eq!(store.load(context)?, Some(initial()?));
        assert_eq!(std::fs::read(&marker)?, original);
        Ok(())
    })
}

#[test]
fn invalid_unknown_and_foreign_records_are_preserved_without_reinitialization() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let initial = initial()?;
        let store = SelectionStore::initialize(root.path(), &initial, context)?;
        for bytes in [b"{broken".to_vec(), initial.encode()?] {
            let bytes = if bytes.starts_with(b"{broken") {
                bytes
            } else {
                let mut value: serde_json::Value = serde_json::from_slice(&bytes)?;
                value["schema_version"] = 99.into();
                serde_json::to_vec(&value)?
            };
            std::fs::write(root.path().join("state").join(DIRECTORY).join(RECORD), &bytes)?;
            assert!(store.load(context).is_err());
            assert!(SelectionStore::initialize(root.path(), &initial, context).is_err());
            assert_eq!(
                std::fs::read(root.path().join("state").join(DIRECTORY).join(RECORD))?,
                bytes
            );
        }
        Ok(())
    })
}

#[test]
fn unrecognized_installation_and_noncontiguous_proposals_are_rejected() -> TestResult {
    worker::run(|context, _| {
        let unknown = tempfile::tempdir()?;
        std::fs::write(unknown.path().join("user-file"), b"preserve")?;
        assert!(SelectionStore::open(unknown.path(), context).is_err());
        assert_eq!(std::fs::read(unknown.path().join("user-file"))?, b"preserve");
        let root = root()?;
        let initial = initial()?;
        let store = SelectionStore::initialize(root.path(), &initial, context)?;
        let candidate = generation(1)?;
        let skipped = initial
            .prepare(candidate.clone(), PreparationIntent::Automatic, PREPARED_AT)?
            .prepared(&candidate)?;
        assert!(matches!(
            store.compare_exchange(&initial, &skipped, context),
            Err(SelectionFailure::Revision)
        ));
        let foreign = InstallationRecord::empty(InstallationId::new("2".repeat(32))?).prepare(
            candidate,
            PreparationIntent::Automatic,
            PREPARED_AT,
        )?;
        assert!(matches!(
            store.compare_exchange(&initial, &foreign, context),
            Err(SelectionFailure::Revision)
        ));
        assert_eq!(store.load(context)?, Some(initial));
        Ok(())
    })
}

#[test]
fn expired_operation_preserves_accepted_record() -> TestResult {
    worker::run_expiring(|context, clock| {
        let root = root()?;
        let initial = initial()?;
        let store = SelectionStore::initialize(root.path(), &initial, context)?;
        let next = initial.prepare(generation(1)?, PreparationIntent::Automatic, PREPARED_AT)?;
        clock.expire();
        assert!(matches!(
            store.compare_exchange(&initial, &next, context),
            Err(SelectionFailure::Storage(_))
        ));
        assert_eq!(
            InstallationRecord::decode(&std::fs::read(
                root.path().join("state").join(DIRECTORY).join(RECORD)
            )?)?,
            initial
        );
        Ok(())
    })
}

#[test]
fn unprotected_namespace_and_earlier_selection_are_preserved_without_adoption() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let directory = root.path().join("state").join(DIRECTORY);
        std::fs::create_dir(&directory)?;
        std::fs::write(directory.join(RECORD), initial()?.encode()?)?;
        assert!(SelectionStore::open(root.path(), context).is_err());
        assert_eq!(std::fs::read(directory.join(RECORD))?, initial()?.encode()?);
        let earlier = super::tests::root()?;
        std::fs::write(earlier.path().join(RECORD), initial()?.encode()?)?;
        assert!(SelectionStore::open(earlier.path(), context).is_err());
        assert!(!earlier.path().join("state").join(DIRECTORY).exists());
        assert_eq!(std::fs::read(earlier.path().join(RECORD))?, initial()?.encode()?);
        Ok(())
    })
}

#[test]
fn unknown_lock_contents_are_preserved_and_not_treated_as_a_stale_owner() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = SelectionStore::initialize(root.path(), &initial()?, context)?;
        let lock = store.root.path().join(LOCK);
        std::fs::write(&lock, b"unknown")?;
        assert!(store.load(context).is_err());
        assert_eq!(std::fs::read(&lock)?, b"unknown");
        Ok(())
    })
}

#[test]
fn inherited_permissions_do_not_replace_explicit_record_protection() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = SelectionStore::initialize(root.path(), &initial()?, context)?;
        let record = store.root.path().join(RECORD);
        std::fs::remove_file(&record)?;
        std::fs::write(&record, initial()?.encode()?)?;
        assert!(store.load(context).is_err());
        assert!(SelectionStore::initialize(root.path(), &initial()?, context).is_err());
        assert_eq!(std::fs::read(&record)?, initial()?.encode()?);
        Ok(())
    })
}

/// Publish each record in order through the real replacement path, returning the last one.
fn advance(
    store: &SelectionStore,
    from: &InstallationRecord,
    steps: impl IntoIterator<Item = InstallationRecord>,
    context: &JobContext,
) -> Result<InstallationRecord, Box<dyn Error + Send + Sync>> {
    let mut current = from.clone();
    for next in steps {
        store.replace(&current, &next, context)?;
        current = next;
    }
    Ok(current)
}

/// Prepare, start and accept `generation` on top of `from`.
fn accept(
    store: &SelectionStore,
    from: &InstallationRecord,
    generation: &Generation,
    context: &JobContext,
) -> Result<InstallationRecord, Box<dyn Error + Send + Sync>> {
    let staged = from.prepare(generation.clone(), PreparationIntent::Automatic, PREPARED_AT)?;
    let prepared = staged.prepared(generation)?;
    let trial = prepared.begin_trial()?;
    let Some(sugarsubstitute_contracts::installation::Candidate::Trial { attempt }) =
        trial.candidate()
    else {
        return Err("Trial absent".into());
    };
    let accepted = trial.accept(attempt)?;
    advance(store, from, [staged, prepared, trial, accepted], context)
}

/// A launch snapshot is a value: a candidate-only replacement after the reservation still
/// selects the same application, a changed accepted generation supersedes it, and a forwarding
/// snapshot never stands in for a launch reservation.
#[test]
fn a_replacement_between_reservation_and_use_is_detected_by_record_value() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let initial = initial()?;
        let store =
            std::sync::Arc::new(SelectionStore::initialize(root.path(), &initial, context)?);
        let reserved = accept(&store, &initial, &generation(1)?, context)?;
        let snapshot_for = |purpose| {
            store.protected(context, |protection| {
                Ok(snapshot::create(&store, protection, &reserved, purpose, context)?)
            })
        };
        let launch = snapshot_for(snapshot::Purpose::Launch)?;
        let forward = snapshot_for(snapshot::Purpose::Forward)?;
        let inherited = |file: &File| -> Result<InheritedSelection, io::Error> {
            Ok(InheritedSelection { file: file.try_clone()? })
        };
        let candidate = advance(
            &store,
            &reserved,
            [reserved.prepare(generation(2)?, PreparationIntent::Automatic, PREPARED_AT)?],
            context,
        )?;
        // Same startup selection: the reservation re-resolves and proceeds to verify the
        // accepted payload, which this record-only fixture does not have.
        let resolved = store.reserve_inherited(Some(inherited(&launch)?), context);
        assert!(matches!(resolved, Err(SelectionFailure::Storage(_))), "{:?}", resolved.err());
        assert!(matches!(
            store.reserve_inherited(Some(inherited(&forward)?), context),
            Err(SelectionFailure::Changed)
        ));
        let cleared = advance(&store, &candidate, [candidate.prepared(&generation(2)?)?], context)?;
        let trial = cleared.begin_trial()?;
        let Some(sugarsubstitute_contracts::installation::Candidate::Trial { attempt }) =
            trial.candidate()
        else {
            return Err("Trial absent".into());
        };
        let accepted = trial.accept(attempt)?;
        advance(&store, &cleared, [trial.clone(), accepted], context)?;
        assert!(matches!(
            store.reserve_inherited(Some(inherited(&launch)?), context),
            Err(SelectionFailure::Superseded)
        ));
        Ok(())
    })
}

/// The update view follows the record: only a complete candidate is prepared, a trial without
/// a verdict returns to prepared, a bad verdict sets rollback, and a later acceptance clears it.
#[test]
fn update_selection_shows_prepared_candidates_and_rollback_only_after_a_bad_verdict() -> TestResult
{
    worker::run(|context, _| {
        let root = root()?;
        let initial = initial()?;
        let store = SelectionStore::initialize(root.path(), &initial, context)?;
        let view = || read_update_selection(root.path(), context);
        assert!(matches!(view(), Err(SelectionFailure::Missing)));
        let running = accept(&store, &initial, &generation(1)?, context)?;
        let shown = view()?;
        assert_eq!(shown.running.generation, generation(1)?.id);
        assert_eq!((shown.running.version.as_str(), shown.running.release_sequence), ("1.0.1", 2));
        assert_eq!((shown.prepared, shown.rollback), (None, None));

        let candidate = generation(2)?;
        let staged =
            running.prepare(candidate.clone(), PreparationIntent::Automatic, PREPARED_AT)?;
        let staged = advance(&store, &running, [staged], context)?;
        assert_eq!(view()?.prepared, None);
        let prepared = advance(&store, &staged, [staged.prepared(&candidate)?], context)?;
        let expected = Some(PreparedUpdate {
            generation: candidate.id.clone(),
            version: "1.0.2".to_owned(),
            release_sequence: 3,
            package_digest: {
                let mut digest = [0; 32];
                digest[31] = 2;
                digest
            },
        });
        assert_eq!(view()?.prepared, expected);

        let trial = prepared.begin_trial()?;
        let Some(Candidate::Trial { attempt }) = trial.candidate().cloned() else {
            return Err("Trial absent".into());
        };
        let trial = advance(&store, &prepared, [trial], context)?;
        assert_eq!(view()?.prepared, None);
        let no_verdict = advance(&store, &trial, [trial.release_trial(&attempt)?], context)?;
        assert_eq!((view()?.prepared, view()?.rollback), (expected, None));

        let trial = advance(&store, &no_verdict, [no_verdict.begin_trial()?], context)?;
        let Some(Candidate::Trial { attempt }) = trial.candidate().cloned() else {
            return Err("Trial absent".into());
        };
        let rejected = advance(&store, &trial, [trial.reject(&attempt, PREPARED_AT)?], context)?;
        let shown = view()?;
        assert_eq!(shown.running.generation, generation(1)?.id);
        assert_eq!(shown.prepared, None);
        assert_eq!(
            shown.rollback,
            Some(Rollback {
                rejected_generation: candidate.id,
                rejected_version: "1.0.2".to_owned(),
                rejected_release_sequence: 3,
            })
        );

        let newest = accept(&store, &rejected, &generation(3)?, context)?;
        let shown = view()?;
        assert_eq!(shown.running.generation, generation(3)?.id);
        assert_eq!((shown.rollback, shown.repairing), (None, None));

        let fallen =
            advance(&store, &newest, [newest.fall_back_to_retained(&generation(3)?)?], context)?;
        let shown = view()?;
        assert_eq!(shown.running.generation, generation(1)?.id);
        assert_eq!(shown.rollback, None);
        assert_eq!(
            shown.repairing,
            Some(InstalledRelease {
                generation: generation(3)?.id,
                version: "1.0.3".to_owned(),
                release_sequence: 4,
            })
        );
        assert_eq!(fallen.repair(), Some(&generation(3)?));
        Ok(())
    })
}
