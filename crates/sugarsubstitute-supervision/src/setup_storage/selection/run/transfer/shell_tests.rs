// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Finish real signed trials and publish shell access without releasing their native run owner.
use super::*;
use crate::setup_storage::{
    InstallationSession, SIGNED_INSTALLER_RESERVATION_BYTES,
    test_worker::{self as worker, TestResult},
};

/// The custody witness saw the coordinator crash before it settled the trial.
const CRASHED: sugarsubstitute_contracts::installation::TrialVerdict =
    sugarsubstitute_contracts::installation::TrialVerdict::Bad(
        sugarsubstitute_contracts::installation::BadTrial::Crashed,
    );

type Failure = Box<dyn std::error::Error + Send + Sync>;
/// Historical shortcut published by the trial fixture's earlier installation.
const LINK: &str = "SugarSubstitute.lnk";
/// Launcher image inside each application generation.
const LAUNCHER: &str = "sugarsubstitute-launcher.exe";
/// Stable root entry users launch.
const ENTRY: &str = "SugarSubstitute.exe";

#[test]
fn retained_trial_publications_fit_their_independent_operation_reservation() -> TestResult {
    let root = tempfile::tempdir()?;
    let path = root.path().to_owned();
    let mut pipeline = worker::pipeline::Pipeline::new()?;
    let application =
        pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| trial(&path, &context))?;
    let (application, accepted) = pipeline.run(
        crate::setup_storage::SELECTION_PUBLICATION_RESERVATION_BYTES,
        move |context| {
            let accepted = application.accept(&context)?;
            Ok((application, accepted))
        },
    )?;
    let application =
        pipeline.run(crate::setup_storage::LAUNCH_RESERVATION_BYTES, move |context| {
            let installed = application.repair_launch_entry(&context)?;
            assert!(installed.executable.is_file());
            application.repair_launch_entry(&context)?;
            Ok(application)
        })?;
    pipeline.run(crate::setup_storage::SELECTION_PUBLICATION_RESERVATION_BYTES, move |context| {
        assert_eq!(application.finish_settled(CRASHED, &context)?, accepted);
        Ok(())
    })
}

/// Use real signatures, files and duplicated custody; the fixture executable is never executed.
pub(super) fn trial(root: &Path, context: &JobContext) -> Result<TrialApplication, Failure> {
    let transfer = crate::setup_storage::test_package::trial_transfer(root, context)?;
    let inherited = InheritedTrial {
        run: File::from(transfer.handles()[0].try_clone_to_owned()?),
        snapshot: File::from(transfer.handles()[1].try_clone_to_owned()?),
    };
    let store = Arc::new(SelectionStore::open(root, context)?);
    let application =
        InstallationRun::receive_trial(store, inherited, transfer.directory(), context)?;
    drop(transfer);
    Ok(application)
}

#[test]
fn trial_shell_publication_requires_health_and_retains_run_custody() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let application = trial(root.path(), context)?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let old_link = std::fs::read(root.path().join(LINK))?;
        assert!(application.repair_launch_entry(context).is_err());
        assert_eq!(std::fs::read(root.path().join(LINK))?, old_link);
        assert!(!root.path().join(ENTRY).exists());
        let accepted = application.accept(context)?;
        let installed = application.repair_launch_entry(context)?;
        assert_eq!(installed.executable, root.path().join(ENTRY));
        assert_eq!(installed.directory, root.path());
        assert!(!root.path().join(LINK).exists());
        assert_eq!(
            std::fs::read(&installed.executable)?,
            std::fs::read(application.directory().join(LAUNCHER))?
        );
        assert_eq!(store.load(context)?, Some(accepted.clone()));
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        assert!(
            std::fs::write(application.directory().join("sugarsubstitute.exe"), b"replace")
                .is_err()
        );
        application.repair_launch_entry(context)?;
        assert_eq!(store.load(context)?, Some(accepted));
        Ok(())
    })
}

#[test]
fn busy_setup_preserves_accepted_trial_and_shell_publication_can_retry() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let application = trial(root.path(), context)?;
        let accepted = application.accept(context)?;
        let store = SelectionStore::open(root.path(), context)?;
        let setup = InstallationSession::maintain(root.path(), context)?;
        let old_link = std::fs::read(root.path().join(LINK))?;
        assert!(application.repair_launch_entry(context).is_err());
        assert_eq!(store.load(context)?, Some(accepted.clone()));
        assert_eq!(std::fs::read(root.path().join(LINK))?, old_link);
        drop(setup);
        application.repair_launch_entry(context)?;
        assert_eq!(store.load(context)?, Some(accepted));
        assert!(root.path().join(ENTRY).is_file());
        Ok(())
    })
}

#[test]
fn unknown_shell_files_cannot_undo_health_or_be_overwritten() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let application = trial(root.path(), context)?;
        let accepted = application.accept(context)?;
        std::fs::write(root.path().join(ENTRY), b"user-owned executable")?;
        let old_link = std::fs::read(root.path().join(LINK))?;
        assert!(application.repair_launch_entry(context).is_err());
        assert_eq!(SelectionStore::open(root.path(), context)?.load(context)?, Some(accepted));
        assert_eq!(std::fs::read(root.path().join(ENTRY))?, b"user-owned executable");
        assert_eq!(std::fs::read(root.path().join(LINK))?, old_link);
        assert!(!root.path().join("state/selection/.launcher-generation.json").exists());
        Ok(())
    })
}

#[test]
fn interruption_after_launcher_intent_reopens_offline_with_accepted_selection() -> TestResult {
    let root = tempfile::tempdir()?;
    let path = root.path().to_owned();
    worker::run_reserved_expiring(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, clock| {
        let application = trial(&path, context)?;
        application.accept(context)?;
        let observed = path.join("state/selection/.launcher-generation.json");
        clock.observe_checks(move || observed.exists())?;
        assert!(application.repair_launch_entry(context).is_err());
        Ok(())
    })?;
    let path = root.path().to_owned();
    worker::run_reserved(crate::setup_storage::LAUNCH_RESERVATION_BYTES, move |context, _| {
        let store = Arc::new(SelectionStore::open(&path, context)?);
        let accepted = store.load(context)?.ok_or("Selection missing")?;
        assert!(accepted.accepted().is_some());
        assert!(accepted.candidate().is_none());
        assert!(path.join("state/selection/.launcher-generation.json").is_file());
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let InstallationLaunch::Application(application) = run.select_launch(None, context)? else {
            return Err("Accepted application could not reopen after shell interruption".into());
        };
        application.repair_launch_entry(context)?;
        assert_eq!(store.load(context)?, Some(accepted));
        assert!(path.join(ENTRY).is_file());
        assert!(!path.join(LINK).exists());
        Ok(())
    })
}
