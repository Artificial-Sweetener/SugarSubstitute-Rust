// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select a provisioning launch contract from authenticated distribution roles.
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_update::package::Manifest;

const NODE: &str = "worker/runtime/node.exe";
const LICENSE: &str = "worker/runtime/LICENSE";
const ELECTRON: &str = "electron/electron.exe";

#[cfg(test)]
mod tests;

/// Runtime and arguments whose bytes remain pinned by the originating verified bundle.
/// Retain that bundle through process creation and the worker's complete lifetime.
pub struct ProvisioningWorker {
    executable: PathBuf,
    directory: PathBuf,
    arguments: Vec<OsString>,
}

impl ProvisioningWorker {
    /// Absolute authenticated runtime path, independent of PATH and installed runtimes.
    pub fn executable(&self) -> &Path {
        &self.executable
    }
    /// Working directory beneath the retained distribution root.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// Fixed runtime options and entry point; append only typed provisioning protocol arguments.
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
}

/// Require one complete canonical runtime; historical packages remain repairable.
pub(in crate::setup_storage) fn uses_node(manifest: &Manifest) -> io::Result<bool> {
    let has = |role: &str| manifest.entries().iter().any(|entry| entry.path() == role);
    let node = manifest
        .entries()
        .iter()
        .any(|entry| entry.path().to_ascii_lowercase().starts_with("worker/runtime/"));
    let electron = manifest
        .entries()
        .iter()
        .any(|entry| entry.path().to_ascii_lowercase().starts_with("electron/"));
    if node == electron || (node && (!has(NODE) || !has(LICENSE))) || (electron && !has(ELECTRON)) {
        return Err(super::super::invalid(
            "Provisioning runtime is absent, ambiguous or noncanonical",
        ));
    }
    Ok(node)
}

/// Reject an offer that would send an unsupported runtime to an older installation engine.
pub(in crate::setup_storage) fn require_engine(
    manifest: &Manifest,
    minimum: u64,
) -> io::Result<()> {
    if uses_node(manifest)? && minimum < 2 {
        return Err(super::super::invalid("Node provisioning packages require updater release 2"));
    }
    Ok(())
}

impl super::VerifiedBundle {
    /// Construct a launch specification only from the retained authenticated inventory.
    /// # Errors
    /// Rejects absent, ambiguous or noncanonical runtime roles.
    pub fn worker(&self) -> io::Result<ProvisioningWorker> {
        let root = &self.payload.root;
        let node = uses_node(&self.payload.inventory)?;
        Ok(ProvisioningWorker {
            executable: root.join(if node { NODE } else { ELECTRON }),
            directory: root.clone(),
            arguments: if node {
                vec![
                    "--use-system-ca".into(),
                    "--use-env-proxy".into(),
                    "--no-global-search-paths".into(),
                    root.join("worker/out/main/index.cjs").into_os_string(),
                ]
            } else {
                vec![root.join("worker").into_os_string()]
            },
        })
    }
}
