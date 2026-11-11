// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stable-entry preparation and failure keep installation selection and run ownership separate.
use super::*;
use crate::{
    application_launch::ApplicationLaunch,
    setup_storage::{ReservedLaunch, reserve_installed_launch},
};

#[test]
fn stable_lookup_does_not_enable_selection_for_an_earlier_installation() -> TestResult {
    worker::run_reserved(crate::setup_storage::LAUNCH_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        fixture::installed(&directory, context)?;
        let transition = directory.path().join("state/selection-transition-lock");
        let original_transition = std::fs::read(&transition)?;
        assert!(matches!(
            reserve_installed_launch(directory.path(), context)?,
            ReservedLaunch::NeedsInstallation
        ));
        for leaf in ["state/selection", "state/selection-enabled.json"] {
            assert!(!directory.path().join(leaf).exists());
        }
        assert_eq!(std::fs::read(transition)?, original_transition);
        Ok(())
    })
}

#[test]
fn prepared_native_launch_retains_selection_until_explicit_retirement_without_taking_run()
-> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        accept_fixture(&directory, context)?;
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let ReservedLaunch::Application(reservation) =
            reserve_installed_launch(directory.path(), context)?
        else {
            return Err("Accepted application absent".into());
        };
        let launch = ApplicationLaunch::selected(reservation.prepare_inheritance(context)?)?;
        let copy = launch.clone();
        drop(launch);
        let run = InstallationRun::acquire(store.clone(), context)?;
        let record = directory.path().join("state/selection/.substitute-selection.json");
        assert!(std::fs::OpenOptions::new().write(true).open(&record).is_err());
        copy.retire()?;
        drop(std::fs::OpenOptions::new().write(true).open(record)?);
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        drop(run);
        assert!(InstallationRun::acquire(store, context).is_ok());
        Ok(())
    })
}

#[test]
fn expired_native_launch_does_not_execute_the_reserved_inert_payload() -> TestResult {
    worker::run_reserved_expiring(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, clock| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        accept_fixture(&directory, context)?;
        let ReservedLaunch::Application(reservation) =
            reserve_installed_launch(directory.path(), context)?
        else {
            return Err("Accepted application absent".into());
        };
        let launch = ApplicationLaunch::selected(reservation.prepare_inheritance(context)?)?;
        let record = directory.path().join("state/selection/.substitute-selection.json");
        let before = std::fs::read(&record)?;
        clock.expire();
        assert!(launch.launch(context).is_err());
        launch.retire()?;
        let _write = std::fs::OpenOptions::new().write(true).open(&record)?;
        assert_eq!(std::fs::read(record)?, before);
        Ok(())
    })
}

#[test]
fn native_creation_failure_is_single_attempt_and_retires_its_original_outputs() -> TestResult {
    worker::run_reserved(1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let absent = temporary.path().join("absent.exe");
        let launch = ApplicationLaunch::new(crate::WindowsLaunchCommand::from_os(
            &absent,
            temporary.path(),
            &[],
        )?);
        assert!(launch.launch(context).is_err());
        assert!(launch.launch(context).is_err());
        launch.retire()?;
        launch.retire()?;
        assert!(!absent.exists());
        Ok(())
    })
}
