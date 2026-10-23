// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real native duplicates qualify entry capture and custody lifetime without executing inert fixtures.
use super::*;
use crate::setup_storage::{
    self as storage, InheritedUpdaterProbe, capture_updater_probe,
    test_package::updater::{archive, evidence_version},
    test_worker::{self as worker, TestResult},
};
use std::os::windows::io::IntoRawHandle;
use sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT;
use sugarsubstitute_update::fixture::NOW;
use windows::Win32::Foundation::{GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};

/// Fund source, destination and child verification against the real execution ledger.
fn run(body: impl FnOnce(&JobContext) -> TestResult + Send + 'static) -> TestResult {
    worker::run_reserved(
        storage::SIGNED_INSTALLER_RESERVATION_BYTES
            + storage::UPDATER_STAGING_RESERVATION_BYTES
            + storage::RESERVATION_BYTES
            + storage::TRUST_OPERATION_RESERVATION_BYTES,
        move |context, _| body(context),
    )
}

fn transfer(
    root: &Path,
    workspace: &Path,
    context: &JobContext,
) -> Result<UpdaterProbeTransfer, Box<dyn std::error::Error + Send + Sync>> {
    Ok(trial(root, workspace, context)?.into_probe(|| NOW, context)?)
}

fn trial(
    root: &Path,
    workspace: &Path,
    context: &JobContext,
) -> Result<UnstartedUpdaterTrial, Box<dyn std::error::Error + Send + Sync>> {
    let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
    let (trust, offer, download) = evidence_version(
        root,
        &bytes,
        Component::Updater,
        sugarsubstitute_update::ENGINE_RELEASE.try_into()?,
        context,
    )?;
    Ok(UpdaterPackage::import(root, trust, offer, download, workspace, || NOW, context)?
        .stage(|| NOW, context)?
        .reserve_trial(context)?)
}

#[test]
fn withdrawn_transfer_rejects_input_composition_without_losing_original_custody() -> TestResult {
    run(|context| {
        let root = worker::installation(166, context)?;
        let workspace = tempfile::tempdir()?;
        let mut probe = transfer(root.path(), workspace.path(), context)?;
        probe.withdraw_before_launch(context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        assert!(matches!(before.candidate(), Some(Candidate::Prepared { .. })));
        let mut probe = Some(probe);
        let owner = sugarsubstitute_contracts::lifecycle::CoordinatorKey {
            namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new([1; 32]),
            epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new([1; 16])?,
        };
        assert!(
            crate::WindowsLaunchInput::updater_probe(
                owner,
                &mut probe,
                crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
            )
            .is_err()
        );
        let probe = probe.as_mut().ok_or("Rejected composition lost its owner")?;
        assert!(!probe.can_launch());
        assert!(trust.recover_updater_trial(context).is_err());
        probe.withdraw_before_launch(context)?;
        assert_eq!(trust.updater_selection(context)?, before);
        Ok(())
    })
}

#[test]
fn rejected_transfer_retains_unstarted_withdrawal_after_expired_authority() -> TestResult {
    run(|context| {
        let root = worker::installation(164, context)?;
        let workspace = tempfile::tempdir()?;
        let mut trial = trial(root.path(), workspace.path(), context)?;
        trial.publish(|| NOW, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        let rejected = match trial.into_probe(|| sugarsubstitute_update::fixture::EXPIRES, context)
        {
            Ok(_) => return Err("Expired transfer unexpectedly succeeded".into()),
            Err(rejected) => rejected,
        };
        assert!(rejected.source().is_some());
        assert!(trust.recover_updater_trial(context).is_err());
        assert_eq!(trust.updater_selection(context)?, before);
        let (mut trial, _cause) = rejected.into_parts();
        trial.restore(context)?;
        assert!(
            matches!(trust.updater_selection(context)?.candidate(), Some(Candidate::Prepared { generation }) if generation == &trial.attempt().generation)
        );
        drop(trial);
        assert!(matches!(
            trust.recover_updater_trial(context)?.candidate(),
            Some(Candidate::Prepared { .. })
        ));
        Ok(())
    })
}

#[test]
fn publication_failure_returns_the_same_owner_for_retry_without_new_generation() -> TestResult {
    run(|context| {
        let root = worker::installation(165, context)?;
        let workspace = tempfile::tempdir()?;
        let trial = trial(root.path(), workspace.path(), context)?;
        let attempt = trial.attempt().clone();
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        let directory = storage::files::Directory::open(&root.path().join(".update-trust"), false)?;
        let pin = directory.read("updater-selection-v1")?;
        let rejected = match trial.into_probe(|| NOW, context) {
            Ok(_) => return Err("Pinned trial publication unexpectedly succeeded".into()),
            Err(rejected) => rejected,
        };
        assert_eq!(trust.updater_selection(context)?, before);
        assert!(trust.recover_updater_trial(context).is_err());
        drop(pin);
        let (trial, _cause) = rejected.into_parts();
        let transfer = trial.into_probe(|| NOW, context)?;
        assert_eq!(transfer.attempt(), &attempt);
        assert!(
            matches!(trust.updater_selection(context)?.candidate(), Some(Candidate::Trial { attempt: recorded }) if recorded == &attempt)
        );
        Ok(())
    })
}

/// Relinquish one genuine duplicate exactly once to simulate the child-side ownership boundary.
fn capture(transfer: &UpdaterProbeTransfer, file: &File) -> io::Result<InheritedUpdaterProbe> {
    let mut arguments = transfer.arguments()?;
    let raw = duplicate_inheritable(file)?.into_raw_handle();
    arguments[3] = (raw as usize).to_string().into();
    // SAFETY: the fixture just relinquished this unique duplicate. No other owner can close or
    // reuse it, and capture receives it exactly once through a complete valid entry grammar.
    unsafe { capture_updater_probe(arguments) }?.ok_or_else(|| io::Error::other("Probe absent"))
}

#[test]
fn child_capture_retains_exclusion_after_creator_loss_and_clears_descendant_inheritance()
-> TestResult {
    run(|context| {
        let root = worker::installation(160, context)?;
        let workspace = tempfile::tempdir()?;
        let transfer = transfer(root.path(), workspace.path(), context)?;
        let executable = transfer.executable();
        let child = capture(&transfer, &transfer.exclusion)?;
        let mut flags = 0;
        // SAFETY: child retains its live uniquely adopted file throughout the read-only query.
        unsafe { GetHandleInformation(HANDLE(child.exclusion.as_raw_handle()), &mut flags) }
            .map_err(io::Error::other)?;
        assert_eq!(flags & HANDLE_FLAG_INHERIT.0, 0);
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        child.verify(&trust, &executable, context)?;
        assert!(trust.recover_updater_trial(context).is_err());
        drop(transfer);
        child.verify(&trust, &executable, context)?;
        assert!(trust.recover_updater_trial(context).is_err());
        assert_eq!(trust.updater_selection(context)?, before);
        drop(child);
        assert!(trust.recover_updater_trial(context)?.candidate().is_none());
        Ok(())
    })
}

#[test]
fn a_foreign_exclusive_file_cannot_authorize_the_named_installations_probe() -> TestResult {
    run(|context| {
        let first = worker::installation(161, context)?;
        let second = worker::installation(162, context)?;
        let workspace = tempfile::tempdir()?;
        let original = transfer(first.path(), workspace.path(), context)?;
        let foreign = transfer(second.path(), workspace.path(), context)?;
        let forged = capture(&original, &foreign.exclusion)?;
        let trust = TrustStore::open(first.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        assert!(forged.verify(&trust, &original.executable(), context).is_err());
        assert_eq!(trust.updater_selection(context)?, before);
        assert!(trust.recover_updater_trial(context).is_err());
        Ok(())
    })
}

#[test]
fn retaining_native_exclusion_does_not_bypass_complete_payload_verification() -> TestResult {
    run(|context| {
        let root = worker::installation(163, context)?;
        let workspace = tempfile::tempdir()?;
        let transfer = transfer(root.path(), workspace.path(), context)?;
        let executable = transfer.executable();
        let child = capture(&transfer, &transfer.exclusion)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        drop(transfer);
        std::fs::write(&executable, b"damaged payload")?;
        assert!(child.verify(&trust, &executable, context).is_err());
        assert_eq!(trust.updater_selection(context)?, before);
        assert_eq!(std::fs::read(executable)?, b"damaged payload");
        Ok(())
    })
}
