// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise transfer with real protected storage, duplicated handles and authenticated packages.
use super::*;
use crate::setup_storage::{InheritedLaunch, capture_launch, retain_running_generation};
use crate::setup_storage::{InheritedTrial, TrialTransfer, UnadoptedEnding, capture_trial_custody};
use std::os::windows::io::{AsRawHandle, BorrowedHandle, IntoRawHandle};
use windows::Win32::{
    Foundation::{
        GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation, WAIT_OBJECT_0,
    },
    System::Threading::WaitForSingleObject,
};

/// Model process inheritance by relinquishing unique native duplicates before initial capture.
fn capture(
    handles: [BorrowedHandle<'_>; 3],
) -> Result<InheritedTrial, Box<dyn std::error::Error + Send + Sync>> {
    let run = handles[0].try_clone_to_owned()?;
    let selection = handles[1].try_clone_to_owned()?;
    let adopted = handles[2].try_clone_to_owned()?;
    for handle in [&run, &selection, &adopted] {
        // SAFETY: every uniquely owned live duplicate is private to this test.
        unsafe {
            SetHandleInformation(
                HANDLE(handle.as_raw_handle()),
                HANDLE_FLAG_INHERIT.0,
                HANDLE_FLAG_INHERIT,
            )
        }?;
    }
    let raw = [
        run.into_raw_handle() as usize,
        selection.into_raw_handle() as usize,
        adopted.into_raw_handle() as usize,
    ];
    // SAFETY: the disjoint raw slots have just relinquished their owners; no other code can
    // close or reuse them. This models the same once-only ownership available at child entry.
    let captured = unsafe {
        capture_launch([
            sugarsubstitute_contracts::application_launch::TRIAL_LAUNCH_ARGUMENT.into(),
            raw[0].to_string().into(),
            raw[1].to_string().into(),
            raw[2].to_string().into(),
        ])
    }?
    .ok_or("Trial capture absent")?;
    let InheritedLaunch::Trial(captured) = captured else {
        return Err("Wrong inherited role".into());
    };
    for slot in raw {
        let mut flags = 0;
        // SAFETY: captured retains the live slot while this query only observes flags.
        unsafe { GetHandleInformation(HANDLE(slot as *mut _), &mut flags) }?;
        assert_eq!(flags & HANDLE_FLAG_INHERIT.0, 0);
    }
    Ok(captured)
}

fn transfer(
    case: &Case<'_>,
) -> Result<(Arc<SelectionStore>, TrialTransfer), Box<dyn std::error::Error + Send + Sync>> {
    let candidate = case.prepared.prepare_update(
        case.runtime,
        &case.signed.authorization(case.trust),
        || NOW,
        case.context,
    )?;
    let store = Arc::new(SelectionStore::open(case.directory.path(), case.context)?);
    let mut run = InstallationRun::acquire(store.clone(), case.context)?;
    let candidate = candidate.begin_trial(
        &mut run,
        &case.signed.authorization(case.trust),
        || NOW,
        case.context,
    )?;
    Ok((store, run.transfer_candidate(candidate, case.context)?))
}

/// Whether the child has told its creator that it holds its own run slot.
fn adopted(transfer: &TrialTransfer) -> bool {
    // SAFETY: the transfer keeps its event handle live; a zero-timeout wait only observes it.
    let state = unsafe { WaitForSingleObject(HANDLE(transfer.adoption().as_raw_handle()), 0) };
    state == WAIT_OBJECT_0
}

/// Inspect only this fixture's transfer snapshots to verify native last-handle cleanup.
fn snapshots(root: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    std::fs::read_dir(root.join("state/selection"))?
        .filter_map(|entry| match entry {
            Ok(entry) if entry.file_name().to_string_lossy().starts_with(".trial-transfer-") => {
                Some(Ok(entry.path()))
            }
            Ok(_) => None,
            Err(cause) => Some(Err(cause)),
        })
        .collect()
}

/// The host's own startup failure, settled by the coordinator.
const BAD: sugarsubstitute_contracts::installation::TrialVerdict =
    sugarsubstitute_contracts::installation::TrialVerdict::Bad(
        sugarsubstitute_contracts::installation::BadTrial::StartupFailed,
    );
#[test]
fn received_trial_keeps_the_run_after_its_creator_leaves_and_accepts_only_once() -> TestResult {
    run(1, |case| {
        let (store, transfer) = transfer(&case)?;
        let directory = transfer.directory().to_owned();
        assert_eq!(transfer.executable(), directory.join("sugarsubstitute.exe"));
        let mut readonly = std::fs::File::from(transfer.handles()[1].try_clone_to_owned()?);
        assert!(std::io::Write::write_all(&mut readonly, b"changed snapshot").is_err());
        drop(readonly);
        let inherited = capture(transfer.handles())?;
        let before = store.load(case.context)?.ok_or("Trial missing")?;
        assert!(InstallationRun::acquire(store.clone(), case.context).is_err());
        assert_eq!(snapshots(case.directory.path())?.len(), 1);
        assert!(!adopted(&transfer));
        let running = retain_running_generation(
            &directory,
            Some(InheritedLaunch::Trial(inherited)),
            case.context,
        )?
        .ok_or("Running trial absent")?;
        assert!(adopted(&transfer));
        // The creator exits once told; the child's own slot keeps every other owner out.
        drop(transfer);
        assert!(InstallationRun::acquire(store.clone(), case.context).is_err());
        assert_eq!(running.installation_id(), before.installation());
        let application = running.trial().ok_or("Running entry lost trial authority")?;
        assert_eq!(application.directory(), directory);
        assert!(snapshots(case.directory.path())?.is_empty());
        assert!(std::fs::write(directory.join("sugarsubstitute.exe"), b"changed").is_err());
        assert_eq!(store.load(case.context)?, Some(before));
        // Storage-boundary health input; inert fixture executable is never launched.
        let accepted = application.accept(case.context)?;
        assert_eq!(accepted.accepted(), Some(&application.attempt().generation));
        assert!(application.accept(case.context).is_err());
        assert_eq!(application.finish_settled(BAD, case.context)?, accepted);
        assert!(InstallationRun::acquire(store.clone(), case.context).is_err());
        drop(running);
        let _run = InstallationRun::acquire(store, case.context)?;
        case.preserved()
    })
}

#[test]
fn receiving_trial_rejects_foreign_native_installation_and_running_generation() -> TestResult {
    run(1, |case| {
        let (store, transfer) = transfer(&case)?;
        let before = store.load(case.context)?;
        assert!(
            InstallationRun::receive_trial(
                store.clone(),
                capture(transfer.handles())?,
                case.original,
                case.context
            )
            .is_err()
        );
        let foreign = tempfile::tempdir()?;
        let foreign_directory = Directory::open(foreign.path(), false)?;
        fixture::installed(&foreign_directory, case.context)?;
        let _trust = case.signed.initialize(foreign.path(), case.context)?;
        let foreign_store = Arc::new(SelectionStore::open(foreign.path(), case.context)?);
        let foreign_before = foreign_store.load(case.context)?;
        assert!(
            InstallationRun::receive_trial(
                foreign_store.clone(),
                capture(transfer.handles())?,
                transfer.directory(),
                case.context
            )
            .is_err()
        );
        assert_eq!(foreign_store.load(case.context)?, foreign_before);
        assert_eq!(store.load(case.context)?, before);
        assert!(InstallationRun::acquire(store, case.context).is_err());
        case.preserved()
    })
}

#[test]
fn received_trial_rejection_preserves_the_previous_accepted_generation() -> TestResult {
    run_accepted(|case| {
        let (store, transfer) = transfer(&case)?;
        let directory = transfer.directory().to_owned();
        let inherited = capture(transfer.handles())?;
        let before = store.load(case.context)?.ok_or("Trial missing")?;
        let application =
            InstallationRun::receive_trial(store.clone(), inherited, &directory, case.context)?;
        drop(transfer);
        // Storage-boundary settlement input; no native process is represented by this fixture.
        let rejected = application.finish_settled(BAD, case.context)?;
        assert_eq!(rejected.accepted(), before.accepted());
        assert_eq!(rejected.rollback(), Some(&application.attempt().generation));
        assert!(
            rejected
                .strikes()
                .iter()
                .any(|strike| strike.package == application.attempt().generation.package)
        );
        assert!(application.accept(case.context).is_err());
        assert!(InstallationRun::acquire(store.clone(), case.context).is_err());
        drop(application);
        let _run = InstallationRun::acquire(store, case.context)?;
        case.preserved()
    })
}

#[test]
fn transfer_rejects_another_live_run_without_changing_either_selection() -> TestResult {
    run(1, |case| {
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        let store = Arc::new(SelectionStore::open(case.directory.path(), case.context)?);
        let mut owner = InstallationRun::acquire(store.clone(), case.context)?;
        let candidate = candidate.begin_trial(
            &mut owner,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        let foreign = tempfile::tempdir()?;
        let foreign_directory = Directory::open(foreign.path(), false)?;
        fixture::installed(&foreign_directory, case.context)?;
        let _trust = case.signed.initialize(foreign.path(), case.context)?;
        let foreign_store = Arc::new(SelectionStore::open(foreign.path(), case.context)?);
        let foreign_run = InstallationRun::acquire(foreign_store.clone(), case.context)?;
        let before = store.load(case.context)?;
        let foreign_before = foreign_store.load(case.context)?;
        assert!(foreign_run.transfer_candidate(candidate, case.context).is_err());
        assert_eq!(store.load(case.context)?, before);
        assert_eq!(foreign_store.load(case.context)?, foreign_before);
        assert!(InstallationRun::acquire(store, case.context).is_err());
        let _foreign_run = InstallationRun::acquire(foreign_store, case.context)?;
        case.preserved()
    })
}

/// A child that gets to run only after its creator gave up, so nobody holds the handoff, never
/// takes the trial, even with an unchanged record and a run nobody holds; its bytes are not
/// trusted either. The record stays as the creator left it for the next launch to resolve.
#[test]
fn a_child_receiving_after_its_creator_left_refuses_the_trial_and_signals_nothing() -> TestResult {
    run(1, |case| {
        let (store, transfer) = transfer(&case)?;
        let directory = transfer.directory().to_owned();
        let inherited = capture(transfer.handles())?;
        let before = store.load(case.context)?;
        let event = transfer.adoption().try_clone_to_owned()?;
        drop(transfer);
        let executable = directory.join("sugarsubstitute.exe");
        std::fs::write(&executable, b"damaged candidate")?;
        assert!(
            InstallationRun::receive_trial(store.clone(), inherited, &directory, case.context)
                .is_err()
        );
        // SAFETY: the duplicated event is owned here; a zero-timeout wait only observes it.
        assert_ne!(unsafe { WaitForSingleObject(HANDLE(event.as_raw_handle()), 0) }, WAIT_OBJECT_0);
        assert_eq!(store.load(case.context)?, before);
        assert_eq!(std::fs::read(executable)?, b"damaged candidate");
        let _run = InstallationRun::acquire(store, case.context)?;
        case.preserved()
    })
}

/// A silent child its creator ended returns the candidate to prepared under the creator's own
/// continuously held run, with nothing against the release, so the next launch tries again.
#[test]
fn an_unadopted_trial_returns_to_prepared_without_counting_against_the_release() -> TestResult {
    run(1, |case| {
        let (store, transfer) = transfer(&case)?;
        let before = store.load(case.context)?.ok_or("Trial missing")?;
        let generation = match before.candidate() {
            Some(sugarsubstitute_contracts::installation::Candidate::Trial { attempt }) => {
                attempt.generation.clone()
            }
            _ => return Err("Trial missing".into()),
        };
        transfer.release_handoff();
        // After the handoff is given up a late child refuses instead of adopting.
        assert!(
            InstallationRun::receive_trial(
                store.clone(),
                capture(transfer.handles())?,
                transfer.directory(),
                case.context
            )
            .is_err()
        );
        assert!(!adopted(&transfer));
        let released = transfer.release_unadopted(UnadoptedEnding::Ended, case.context)?;
        assert!(matches!(
            released.candidate(),
            Some(sugarsubstitute_contracts::installation::Candidate::Prepared { generation: prepared })
                if prepared == &generation
        ));
        assert_eq!(released.accepted(), before.accepted());
        assert!(released.strikes().is_empty());
        assert!(released.unwitnessed().is_none());
        assert_eq!(store.load(case.context)?, Some(released.clone()));
        // Repeating the release keeps the first result while the creator still holds the run.
        assert_eq!(transfer.release_unadopted(UnadoptedEnding::Ended, case.context)?, released);
        assert_eq!(transfer.release_unadopted(UnadoptedEnding::Exited, case.context)?, released);
        assert!(InstallationRun::acquire(store.clone(), case.context).is_err());
        drop(transfer);
        let _run = InstallationRun::acquire(store, case.context)?;
        case.preserved()
    })
}

#[test]
fn initial_trial_capture_rejects_aliased_roles_without_closing_the_handle() -> TestResult {
    let file = tempfile::tempfile()?;
    let raw = file.as_raw_handle() as usize;
    let other = tempfile::tempfile()?;
    let second = other.as_raw_handle() as usize;
    // The test retains the sole owners, and checks that each rejected ambiguous request leaves
    // them usable.
    for slots in [[raw, raw, second], [raw, second, raw], [second, raw, raw]] {
        // SAFETY: any repeated slot is rejected before adoption or native inspection.
        assert!(unsafe { capture_trial_custody(slots) }.is_err());
    }
    assert_eq!(other.metadata()?.len(), 0);
    assert_eq!(file.metadata()?.len(), 0);
    Ok(())
}

#[test]
fn acceptance_does_not_wait_for_creator_retirement_and_old_transfer_cannot_be_replayed()
-> TestResult {
    run(1, |case| {
        let (store, transfer) = transfer(&case)?;
        let before = store.load(case.context)?;
        let application = InstallationRun::receive_trial(
            store.clone(),
            capture(transfer.handles())?,
            transfer.directory(),
            case.context,
        )?;
        let accepted = application.accept(case.context)?;
        assert_eq!(accepted.accepted(), Some(&application.attempt().generation));
        assert_ne!(store.load(case.context)?, before);
        assert!(
            InstallationRun::receive_trial(
                store.clone(),
                capture(transfer.handles())?,
                transfer.directory(),
                case.context,
            )
            .is_err()
        );
        assert!(InstallationRun::acquire(store.clone(), case.context).is_err());
        drop(transfer);
        assert!(InstallationRun::acquire(store, case.context).is_err());
        case.preserved()
    })
}

/// A child that exited on its own before taking over also returns the candidate to prepared,
/// but counts toward the abandoned-trial streak, so a release that never starts is not retried
/// at every launch forever.
#[test]
fn a_child_exiting_before_it_took_over_counts_toward_the_unwitnessed_streak() -> TestResult {
    run(1, |case| {
        let (store, transfer) = transfer(&case)?;
        let before = store.load(case.context)?.ok_or("Trial missing")?;
        let released = transfer.release_unadopted(UnadoptedEnding::Exited, case.context)?;
        assert!(matches!(
            released.candidate(),
            Some(sugarsubstitute_contracts::installation::Candidate::Prepared { .. })
        ));
        assert_eq!(released.accepted(), before.accepted());
        assert!(released.strikes().is_empty());
        assert_eq!(released.unwitnessed().map(|streak| streak.count), Some(1));
        assert_eq!(transfer.release_unadopted(UnadoptedEnding::Exited, case.context)?, released);
        drop(transfer);
        let _run = InstallationRun::acquire(store, case.context)?;
        case.preserved()
    })
}

#[test]
fn package_without_trial_entry_is_rejected_before_persisting_an_attempt() -> TestResult {
    run_distribution(
        1,
        false,
        test_package::distribution_with_contract(Some(
            br#"{"schema_version":1,"installed_launch_protocols":[3]}"#,
        ))?,
        |case| {
            let candidate = case.prepared.prepare_update(
                case.runtime,
                &case.signed.authorization(case.trust),
                || NOW,
                case.context,
            )?;
            let store = Arc::new(SelectionStore::open(case.directory.path(), case.context)?);
            let before = store.load(case.context)?;
            let mut run = InstallationRun::acquire(store.clone(), case.context)?;
            assert!(
                candidate
                    .begin_trial(
                        &mut run,
                        &case.signed.authorization(case.trust),
                        || NOW,
                        case.context
                    )
                    .is_err()
            );
            assert_eq!(store.load(case.context)?, before);
            case.preserved()
        },
    )
}

#[test]
fn transfer_snapshot_detects_record_tampering_and_is_deleted_after_rejection() -> TestResult {
    run(1, |case| {
        let (store, transfer) = transfer(&case)?;
        let inherited = capture(transfer.handles())?;
        let directory = transfer.directory().to_owned();
        let record = case.directory.path().join("state/selection/.substitute-selection.json");
        let original = std::fs::read(&record)?;
        let mut changed: serde_json::Value = serde_json::from_slice(&original)?;
        changed["installation"] = "f".repeat(32).into();
        let changed = serde_json::to_vec(&changed)?;
        std::fs::write(&record, &changed)?;
        assert!(
            InstallationRun::receive_trial(store, inherited, &directory, case.context).is_err()
        );
        assert!(!adopted(&transfer));
        drop(transfer);
        assert_eq!(std::fs::read(record)?, changed);
        assert!(snapshots(case.directory.path())?.is_empty());
        case.preserved()
    })
}
