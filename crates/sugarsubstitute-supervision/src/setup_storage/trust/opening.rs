// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Open established local authority independently of network discovery configuration.
use super::*;

#[cfg(test)]
mod tests;

impl TrustStore {
    /// Reopen the installation's protected authority without a network distribution.
    /// The checkpoint, release history and existing continuity marker must agree on
    /// installation and repository identity. This never bootstraps missing authority.
    /// # Errors
    /// Rejects missing, damaged, redirected, foreign or unprotected records. Execute on
    /// an admitted storage worker with `TRUST_RESERVATION_BYTES` available.
    pub fn open_installed(
        installation_root: &Path,
        context: &JobContext,
    ) -> Result<Self, TrustFailure> {
        Self::open_bound(installation_root, None, context)
    }

    /// Require the distribution's repository for online work; offline work requires
    /// already established continuity instead of creating a migration marker.
    pub(super) fn open_bound(
        installation_root: &Path,
        expected: Option<[u8; 32]>,
        context: &JobContext,
    ) -> Result<Self, TrustFailure> {
        check(context)?;
        let selection = Self::selection(installation_root, context)?;
        let memory = context.memory().claim(1024 * 1024).map_err(io::Error::other)?;
        let _work = context.memory().claim(WORK_BYTES).map_err(io::Error::other)?;
        let parent = Directory::open(installation_root, false)?;
        let ownership = ownership::read(&parent)?;
        let root = Directory::open(&parent.path().join(DIRECTORY), false)?;
        protection::with_domain(context, |protection| {
            protection.directory(root.path(), Mode::Existing, context)?;
            let writer = root.read(LOCK)?;
            protection.verify_file(&writer)?;
            writer.try_lock().map_err(io::Error::from)?;
            let mut file = root.read(RECORD)?;
            protection.verify_file(&file)?;
            let bytes = files::read_bounded(&mut file, envelope::Kind::Checkpoint.limit())?;
            let (repository, checkpoint) = envelope::decode_binding(
                envelope::Kind::Checkpoint,
                selection.installation(),
                &bytes,
            )?;
            if expected.is_some_and(|expected| expected != repository) {
                return Err(invalid("Update trust repository differs from distribution").into());
            }
            RepositoryTrust::from_trusted_checkpoint(checkpoint)?;
            let mut store = Self {
                cache: std::sync::Mutex::new(None),
                preparation: None,
                root,
                _installation_root: parent,
                _ownership: ownership,
                _continuity: None,
                _memory: memory,
                workspace: context.memory().clone(),
                installation: selection.installation().clone(),
                repository,
            };
            store.read_releases(protection)?;
            let marker = if expected.is_some() {
                continuity::ensure(&store, protection, context)?
            } else {
                continuity::read(&store, protection)?
                    .ok_or_else(|| invalid("Established update trust continuity is absent"))?
            };
            check(context)?;
            store._continuity = Some(marker);
            Ok(store)
        })
        .map_err(TrustFailure::from)
    }
}
