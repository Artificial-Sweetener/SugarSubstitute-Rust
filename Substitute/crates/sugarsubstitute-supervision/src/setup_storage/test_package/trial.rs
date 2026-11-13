// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit an inert signed trial through the production installer pipeline for native tests.
use super::repository::SignedPackage;
use crate::{
    durable_storage::files::Directory,
    setup_storage::{
        InheritedLaunch, InheritedTrial, InstallationSession, InstallerSource, TrialTransfer,
        capture_launch, existing::fixture, test_package,
    },
};
use std::{
    os::windows::io::{AsRawHandle, BorrowedHandle, IntoRawHandle},
    path::Path,
};
use sugarsubstitute_contracts::{
    application_launch::{CURRENT_CONTRACT, CURRENT_LAUNCHER_CONTRACT},
    execution::JobContext,
};
use sugarsubstitute_update::fixture::NOW;
use windows::Win32::Foundation::{
    GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation,
};

/// Produce actual run/snapshot transfer custody; no fixture executable is run.
pub(crate) fn trial_transfer(
    root: &Path,
    context: &JobContext,
) -> Result<TrialTransfer, Box<dyn std::error::Error + Send + Sync>> {
    let directory = Directory::open(root, false)?;
    fixture::installed(&directory, context)?;
    let workspace = tempfile::tempdir()?;
    let (manifest, compressed) = test_package::distribution_with_launcher(
        Some(CURRENT_CONTRACT),
        Some(CURRENT_LAUNCHER_CONTRACT),
    )?;
    let archive = test_package::seal(&manifest, &compressed);
    let signed = SignedPackage::new(&archive, 1)?;
    let executable = workspace.path().join("setup.exe");
    std::fs::write(&executable, [b"inert installer prefix".as_slice(), &archive].concat())?;
    let imported = InstallationSession::maintain(root, context)?.import_installer(
        InstallerSource {
            repository: [92; 32],
            trusted_root: &signed.root,
            roots: &[],
            metadata: signed.metadata.metadata(),
            descriptor: &signed.descriptor,
            executable: &executable,
        },
        signed.request,
        crate::setup_storage::BundleSource::Extract(workspace.path()),
        || NOW,
        context,
    )?;
    let prepared = imported.prepare(|| NOW, context)?;
    let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
    Ok(prepared.complete(&runtime, || NOW, context)?.transfer(|| NOW, context, &mut None)?)
}

/// Model process inheritance by relinquishing unique native duplicates before initial capture,
/// as the started copy receives a trial's `handles`.
pub(crate) fn inherit(
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
