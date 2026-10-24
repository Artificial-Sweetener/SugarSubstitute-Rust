// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retained real engine custody survives failed creation without launching inert fixture bytes.
use super::*;
mod cleanup;
mod fixture;
mod native_lifetime;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use crate::setup_storage::{
    self as storage, TrustStore, UpdaterPackage,
    test_package::updater::{accept_prepared, archive, evidence, run},
};
use sugarsubstitute_contracts::{
    application_launch::updater::CURRENT_UPDATER_CONTRACT,
    update::{Channel, Component},
};
use sugarsubstitute_update::fixture::NOW;

#[test]
fn failed_engine_creation_retires_pins_without_restoring_or_changing_application_state()
-> TestResult {
    run(|context| {
        let root = worker::installation(191, context)?;
        let workspace = tempfile::tempdir()?;
        let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
        let (trust, offer, download) = evidence(root.path(), &bytes, Component::Updater, context)?;
        let prepared = UpdaterPackage::import(
            root.path(),
            trust,
            offer,
            download,
            workspace.path(),
            || NOW,
            context,
        )?
        .stage(|| NOW, context)?;
        accept_prepared(prepared, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        let application = storage::SelectionStore::open(root.path(), context)?;
        let before_application = application.load(context)?;
        let image = trust.pin_updater(Channel::Release, 2, context)?.ok_or("Engine absent")?;
        let executable = image.executable();
        let launch = ActivationLaunch::new(Box::new(image));
        launch.prepare(context)?;
        assert!(launch.prepare(context).is_err());
        assert!(
            launch
                .launch_with(
                    context,
                    |command, job, channel, _| {
                        assert!(!job.is_invalid());
                        assert_ne!(channel, 0);
                        let text = String::from_utf16_lossy(&command.command_line);
                        assert!(text.contains(ACTIVATION_ARGUMENT));
                        Err(ClaimError::invalid("fixture_create_rejected"))
                    },
                    native::resume
                )
                .is_err()
        );
        assert_eq!(
            launch.poll(context, true)?,
            ActivationOutcome::Stopped { exit_code: None, creation: CreationOutcome::NotCreated }
        );
        assert!(std::fs::OpenOptions::new().write(true).open(executable).is_ok());
        assert_eq!(trust.updater_selection(context)?, before);
        assert_eq!(application.load(context)?, before_application);
        Ok(())
    })
}

#[test]
fn historical_engine_capability_rejection_keeps_custody_until_explicit_retirement() -> TestResult {
    run(|context| {
        let root = worker::installation(192, context)?;
        let workspace = tempfile::tempdir()?;
        let (bytes, _) = archive(br#"{"schema_version":2,"preparation_protocols":[1],"installation_schemas":[1],"probe_protocols":[2]}"#)?;
        let (trust, offer, download) = evidence(root.path(), &bytes, Component::Updater, context)?;
        let prepared = UpdaterPackage::import(
            root.path(),
            trust,
            offer,
            download,
            workspace.path(),
            || NOW,
            context,
        )?
        .stage(|| NOW, context)?;
        accept_prepared(prepared, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let image = trust.pin_updater(Channel::Release, 2, context)?.ok_or("Engine absent")?;
        let executable = image.executable();
        let launch = ActivationLaunch::new(Box::new(image));
        assert!(launch.prepare(context).is_err());
        assert!(std::fs::OpenOptions::new().write(true).open(&executable).is_err());
        assert_eq!(
            launch.poll(context, true)?,
            ActivationOutcome::Stopped { exit_code: None, creation: CreationOutcome::NotCreated }
        );
        assert!(std::fs::OpenOptions::new().write(true).open(executable).is_ok());
        Ok(())
    })
}
