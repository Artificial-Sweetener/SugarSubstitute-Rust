// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit an inert signed trial through the production installer pipeline for native tests.
use super::repository::SignedPackage;
use crate::setup_storage::{
    InstallationSession, InstallerSource, TrialTransfer, existing::fixture, files::Directory,
    test_package,
};
use std::path::Path;
use sugarsubstitute_contracts::{
    application_launch::{CURRENT_CONTRACT, CURRENT_LAUNCHER_CONTRACT},
    execution::JobContext,
};
use sugarsubstitute_update::fixture::NOW;

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
        workspace.path(),
        || NOW,
        context,
    )?;
    let prepared = imported.prepare(|| NOW, context)?;
    let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
    Ok(prepared.complete(&runtime, || NOW, context)?.transfer(|| NOW, context, &mut None)?)
}
