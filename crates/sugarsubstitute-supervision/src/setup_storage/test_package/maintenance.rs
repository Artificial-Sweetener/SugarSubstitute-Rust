// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared inert native installation for signed installer and preparation-worker qualification.
use crate::setup_storage::{
    existing::fixture,
    files::Directory,
    test_package::{self, repository::SignedPackage},
    test_worker::TestResult,
    *,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_update::fixture::NOW;
type Failure = Box<dyn std::error::Error + Send + Sync>;

/// Supply a health result through the real native run owner for inert prepared test payloads.
/// This fixture boundary starts no process and does not qualify executable readiness.
pub(crate) fn accept_pending(root: &Path, context: &JobContext) -> TestResult {
    let store = Arc::new(SelectionStore::open(root, context)?);
    let mut run = InstallationRun::acquire(store.clone(), context)?;
    let trial = run.begin_trial(&store.load(context)?.ok_or("Selection missing")?, context)?;
    run.accept(&trial, context)?;
    Ok(())
}

/// Existing native choices and exact package bytes; no payload in this fixture is executed.
pub(crate) struct Fixture {
    pub(crate) installation: tempfile::TempDir,
    pub(crate) workspace: tempfile::TempDir,
    pub(crate) source: PathBuf,
    pub(crate) original: PathBuf,
    pub(crate) archive: Vec<u8>,
}
impl Fixture {
    /// Create actual preserved startup choices and a complete inert application archive.
    pub(crate) fn new(context: &JobContext) -> Result<Self, Failure> {
        Self::with_distribution(test_package::distribution()?, context)
    }

    /// Install a caller-selected inert package through the same signed import boundary.
    pub(crate) fn with_distribution(
        distribution: (Vec<u8>, Vec<u8>),
        context: &JobContext,
    ) -> Result<Self, Failure> {
        let installation = tempfile::tempdir()?;
        let workspace = tempfile::tempdir()?;
        let directory = Directory::open(installation.path(), false)?;
        let original = fixture::installed(&directory, context)?;
        let (manifest, compressed) = distribution;
        let archive = test_package::seal(&manifest, &compressed);
        let source = workspace.path().join("setup.exe");
        std::fs::write(&source, [b"inert installer prefix".as_slice(), &archive].concat())?;
        Ok(Self { installation, workspace, source, original, archive })
    }

    /// Exercise the public signed offline import with this fixture's original paths.
    pub(crate) fn import(
        &self,
        signed: SignedPackage,
        context: &JobContext,
    ) -> Result<SignedInstallation, TrustFailure> {
        InstallationSession::maintain(self.installation.path(), context)?.import_installer(
            InstallerSource {
                repository: [92; 32],
                trusted_root: &signed.root,
                roots: &[],
                metadata: signed.metadata.metadata(),
                descriptor: &signed.descriptor,
                executable: &self.source,
            },
            signed.request,
            crate::setup_storage::BundleSource::Extract(self.workspace.path()),
            || NOW,
            context,
        )
    }

    /// Check authored configuration and the absence of unexpected ComfyUI provisioning.
    pub(crate) fn preserved(&self) -> TestResult {
        assert_eq!(
            std::fs::read(self.original.join("startup.local.toml"))?,
            b"schema_version = 1\r\n# User customization\r\n[backend]\r\nport = 9191\r\n"
        );
        assert!(!self.installation.path().join("external-comfy").exists());
        assert!(!self.installation.path().join("python.exe").exists());
        Ok(())
    }
}
