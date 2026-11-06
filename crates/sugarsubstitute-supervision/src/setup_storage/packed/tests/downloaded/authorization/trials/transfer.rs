// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise transfer with real protected storage, duplicated handles and authenticated packages.
use super::*;
use crate::setup_storage::{InheritedLaunch, capture_launch, retain_running_generation};
use crate::setup_storage::{InheritedTrial, TrialTransfer, capture_trial_custody};
use std::os::windows::io::{AsRawHandle, BorrowedHandle, IntoRawHandle};
use windows::Win32::Foundation::{
    GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation,
};

/// Model process inheritance by relinquishing unique native duplicates before initial capture.
fn capture(
    handles: [BorrowedHandle<'_>; 2],
) -> Result<InheritedTrial, Box<dyn std::error::Error + Send + Sync>> {
    let run = handles[0].try_clone_to_owned()?;
    let selection = handles[1].try_clone_to_owned()?;
    for handle in [&run, &selection] {
        // SAFETY: both uniquely owned live duplicates are private to this test.
        unsafe {
            SetHandleInformation(
                HANDLE(handle.as_raw_handle()),
                HANDLE_FLAG_INHERIT.0,
                HANDLE_FLAG_INHERIT,
            )
        }?;
    }
    let raw = [run.into_raw_handle() as usize, selection.into_raw_handle() as usize];
    // SAFETY: both disjoint raw slots have just relinquished their owners; no other code can
    // close or reuse them. This models the same once-only ownership available at child entry.
    let captured = unsafe {
        capture_launch([
            sugarsubstitute_contracts::application_launch::TRIAL_LAUNCH_ARGUMENT.into(),
            raw[0].to_string().into(),
            raw[1].to_string().into(),
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

#[test]
fn receiving_trial_after_creator_drop_retains_payloads_and_accepts_only_once() -> TestResult {
    run(1, |case| {
        let (store, transfer) = transfer(&case)?;
        let directory = transfer.directory().to_owned();
        assert_eq!(transfer.executable(), directory.join("sugarsubstitute.exe"));
        let mut readonly = std::fs::File::from(transfer.handles()[1].try_clone_to_owned()?);
        assert!(std::io::Write::write_all(&mut readonly, b"changed snapshot").is_err());
        drop(readonly);
        let inherited = capture(transfer.handles())?;
        let before = store.load(case.context)?.ok_or("Trial missing")?;
        drop(transfer);
        assert!(InstallationRun::acquire(store.clone(), case.context).is_err());
        assert_eq!(snapshots(case.directory.path())?.len(), 1);
        let running = retain_running_generation(
            &directory,
            Some(InheritedLaunch::Trial(inherited)),
            case.context,
        )?
        .ok_or("Running trial absent")?;
        let application = running.trial().ok_or("Running entry lost trial authority")?;
        assert_eq!(application.directory(), directory);
        assert!(snapshots(case.directory.path())?.is_empty());
        assert!(std::fs::write(directory.join("sugarsubstitute.exe"), b"changed").is_err());
        assert_eq!(store.load(case.context)?, Some(before));
        // Storage-boundary health input; inert fixture executable is never launched.
        let accepted = application.accept(case.context)?;
        assert_eq!(accepted.accepted(), Some(&application.attempt().generation));
        assert!(application.accept(case.context).is_err());
        assert!(application.reject(case.context).is_err());
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
        drop(transfer);
        let application =
            InstallationRun::receive_trial(store.clone(), inherited, &directory, case.context)?;
        // Storage-boundary settlement input; no native process is represented by this fixture.
        let rejected = application.reject(case.context)?;
        assert_eq!(rejected.accepted(), before.accepted());
        assert!(rejected.rejected().contains(&application.attempt().generation.package));
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

#[test]
fn received_trial_rechecks_payloads_after_creator_releases_its_pins() -> TestResult {
    run(1, |case| {
        let (store, transfer) = transfer(&case)?;
        let directory = transfer.directory().to_owned();
        let inherited = capture(transfer.handles())?;
        let before = store.load(case.context)?;
        drop(transfer);
        let executable = directory.join("sugarsubstitute.exe");
        std::fs::write(&executable, b"damaged candidate")?;
        assert!(
            InstallationRun::receive_trial(store.clone(), inherited, &directory, case.context)
                .is_err()
        );
        assert_eq!(store.load(case.context)?, before);
        assert_eq!(std::fs::read(executable)?, b"damaged candidate");
        let _run = InstallationRun::acquire(store, case.context)?;
        case.preserved()
    })
}

#[test]
fn initial_trial_capture_rejects_aliased_roles_without_closing_the_handle() -> TestResult {
    let file = tempfile::tempfile()?;
    let raw = file.as_raw_handle() as usize;
    // SAFETY: equal slots are rejected before adoption or native inspection. The test retains
    // the sole owner, and checks that this rejected ambiguous request leaves it usable.
    assert!(unsafe { capture_trial_custody([raw, raw]) }.is_err());
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

#[test]
fn package_without_trial_entry_is_rejected_before_persisting_an_attempt() -> TestResult {
    run_distribution(
        1,
        false,
        test_package::distribution_with_contract(Some(
            br#"{"schema_version":1,"installed_launch_protocols":[1]}"#,
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
        drop(transfer);
        let record = case.directory.path().join("state/selection/.substitute-selection.json");
        let original = std::fs::read(&record)?;
        let mut changed: serde_json::Value = serde_json::from_slice(&original)?;
        changed["installation"] = "f".repeat(32).into();
        let changed = serde_json::to_vec(&changed)?;
        std::fs::write(&record, &changed)?;
        assert!(
            InstallationRun::receive_trial(store, inherited, &directory, case.context).is_err()
        );
        assert_eq!(std::fs::read(record)?, changed);
        assert!(snapshots(case.directory.path())?.is_empty());
        case.preserved()
    })
}
