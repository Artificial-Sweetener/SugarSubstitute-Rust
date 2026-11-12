// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native file-handle custody survives sender retirement without reconstructing a trial.
use super::*;
use crate::setup_storage::{
    SIGNED_INSTALLER_RESERVATION_BYTES,
    test_worker::{self as worker, TestResult},
};
use std::io::Write;

/// The custody witness saw the coordinator crash before it settled the trial.
const CRASHED: sugarsubstitute_contracts::installation::TrialVerdict =
    sugarsubstitute_contracts::installation::TrialVerdict::Bad(
        sugarsubstitute_contracts::installation::BadTrial::Crashed,
    );

/// Duplicate actual native transfer handles; no persisted record substitutes for custody.
fn inherited(transfer: &TrialWitnessTransfer) -> io::Result<InheritedTrial> {
    Ok(InheritedTrial {
        run: File::from(transfer.handles()[0].try_clone_to_owned()?),
        snapshot: File::from(transfer.handles()[1].try_clone_to_owned()?),
    })
}

#[test]
fn witness_retains_native_exclusion_after_all_sender_custody_is_dropped() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let application = shell_tests::trial(root.path(), context)?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let original = store.load(context)?.ok_or("Selection absent")?;
        let transfer = application.prepare_witness(context)?;
        let captured = inherited(&transfer)?;
        let generation = application.directory().to_owned();
        let attempt = application.attempt().clone();
        drop(application);
        drop(transfer);
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        let witness = TrialWitness::receive(store.clone(), captured, &generation, context)?;
        assert_eq!(witness.attempt(), &attempt);
        assert_eq!(store.load(context)?, Some(original.clone()));
        assert!(std::fs::rename(&generation, generation.with_extension("moved")).is_err());
        let rejected = witness.finish_settled(CRASHED, context)?;
        assert_eq!(
            rejected,
            original.reject(&attempt, rejected.strikes().first().ok_or("Strike absent")?.at)?
        );
        assert_eq!(witness.finish_settled(CRASHED, context)?, rejected);
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        drop(witness);
        let _successor = InstallationRun::acquire(store.clone(), context)?;
        assert_eq!(store.load(context)?, Some(rejected));
        Ok(())
    })
}

/// Task Manager or a forced shutdown ending the coordinator is not the release's fault: the
/// witness returns the candidate to prepared, so the next launch tries it again.
#[test]
fn witness_returns_a_trial_whose_coordinator_was_ended_from_outside_to_prepared() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let application = shell_tests::trial(root.path(), context)?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let transfer = application.prepare_witness(context)?;
        let witness = TrialWitness::receive(
            store.clone(),
            inherited(&transfer)?,
            application.directory(),
            context,
        )?;
        let attempt = application.attempt().clone();
        drop(application);
        drop(transfer);
        let ended = sugarsubstitute_contracts::installation::HostExit::Ended(1).verdict();
        let released = witness.finish_settled(ended, context)?;
        assert_eq!(
            released.candidate(),
            Some(&Candidate::Prepared { generation: attempt.generation.clone() })
        );
        assert!(released.strikes().is_empty());
        assert!(released.rollback().is_none());
        assert_eq!(witness.finish_settled(CRASHED, context)?, released);
        assert_eq!(store.load(context)?, Some(released));
        Ok(())
    })
}

#[test]
fn witness_preserves_coordinator_acceptance_after_sender_retirement() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let application = shell_tests::trial(root.path(), context)?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let transfer = application.prepare_witness(context)?;
        let witness = TrialWitness::receive(
            store.clone(),
            inherited(&transfer)?,
            application.directory(),
            context,
        )?;
        let accepted = application.accept(context)?;
        drop(application);
        drop(transfer);
        assert_eq!(witness.finish_settled(CRASHED, context)?, accepted);
        assert_eq!(witness.finish_settled(CRASHED, context)?, accepted);
        assert_eq!(store.load(context)?, Some(accepted));
        assert!(InstallationRun::acquire(store, context).is_err());
        Ok(())
    })
}

#[test]
fn witness_rejects_cross_installation_mixed_handles_and_wrong_generation() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let first = tempfile::tempdir()?;
        let second = tempfile::tempdir()?;
        let a = shell_tests::trial(first.path(), context)?;
        let a_directory = a.directory().to_owned();
        let a_transfer = a.prepare_witness(context)?;
        drop(a);
        let b = shell_tests::trial(second.path(), context)?;
        let b_directory = b.directory().to_owned();
        let b_transfer = b.prepare_witness(context)?;
        drop(b);
        let a_store = Arc::new(SelectionStore::open(first.path(), context)?);
        let b_store = Arc::new(SelectionStore::open(second.path(), context)?);
        let a_original = a_store.load(context)?;
        let b_original = b_store.load(context)?;
        assert!(
            TrialWitness::receive(b_store.clone(), inherited(&a_transfer)?, &b_directory, context)
                .is_err()
        );
        let mixed = InheritedTrial {
            run: File::from(a_transfer.handles()[0].try_clone_to_owned()?),
            snapshot: File::from(b_transfer.handles()[1].try_clone_to_owned()?),
        };
        assert!(TrialWitness::receive(a_store.clone(), mixed, &a_directory, context).is_err());
        assert!(
            TrialWitness::receive(a_store.clone(), inherited(&a_transfer)?, &b_directory, context)
                .is_err()
        );
        let swapped = InheritedTrial {
            run: File::from(a_transfer.handles()[1].try_clone_to_owned()?),
            snapshot: File::from(a_transfer.handles()[0].try_clone_to_owned()?),
        };
        assert!(TrialWitness::receive(a_store.clone(), swapped, &a_directory, context).is_err());
        assert_eq!(a_store.load(context)?, a_original);
        assert_eq!(b_store.load(context)?, b_original);
        Ok(())
    })
}

#[test]
fn witness_snapshot_is_readonly_and_enrollment_requires_unresolved_selection() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let application = shell_tests::trial(root.path(), context)?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let transfer = application.prepare_witness(context)?;
        let mut snapshot = File::from(transfer.handles()[1].try_clone_to_owned()?);
        assert!(snapshot.write_all(b"replace").is_err());
        let accepted = application.accept(context)?;
        assert!(matches!(application.prepare_witness(context), Err(SelectionFailure::Changed)));
        assert!(matches!(
            TrialWitness::receive(
                store.clone(),
                inherited(&transfer)?,
                application.directory(),
                context
            ),
            Err(SelectionFailure::Changed)
        ));
        assert_eq!(store.load(context)?, Some(accepted));
        Ok(())
    })
}

#[test]
fn lost_witness_finalization_result_replays_without_reacquiring_run_ownership() -> TestResult {
    let retained = Arc::new(std::sync::Mutex::new(None));
    let output = retained.clone();
    worker::run_reserved_expiring(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, clock| {
        let root = tempfile::tempdir()?;
        let application = shell_tests::trial(root.path(), context)?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let transfer = application.prepare_witness(context)?;
        let witness = TrialWitness::receive(
            store.clone(),
            inherited(&transfer)?,
            application.directory(),
            context,
        )?;
        drop(application);
        drop(transfer);
        let finalized = witness.finish_settled(CRASHED, context)?;
        *output.lock().map_err(|_| "Retention poisoned")? = Some((root, store, witness, finalized));
        clock.expire();
        Ok(())
    })?;
    worker::run(move |context, _| {
        let state = retained.lock().map_err(|_| "Retention poisoned")?;
        let (_root, store, witness, finalized) = state.as_ref().ok_or("Witness custody lost")?;
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        assert_eq!(&witness.finish_settled(CRASHED, context)?, finalized);
        assert_eq!(store.load(context)?.as_ref(), Some(finalized));
        Ok(())
    })
}
