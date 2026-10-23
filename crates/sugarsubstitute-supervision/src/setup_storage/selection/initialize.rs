// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish complete revision-zero state before enabling its durable continuity marker.
use super::*;

impl SelectionStore {
    /// Initialize a previously absent selection with one atomic directory publication.
    /// The initial record must be revision zero. Staging is never read as authority or reused.
    /// If publication completed before interruption, reopen the store instead of initializing it.
    /// # Errors
    /// Rejects every occupied namespace or continuity marker, including incomplete state.
    /// Preserves unknown files, abandoned staging and any already published selection.
    pub fn initialize(
        installation_root: &Path,
        initial: &InstallationRecord,
        context: &JobContext,
    ) -> Result<Self, SelectionFailure> {
        check(context)?;
        if initial.revision() != 0 {
            return Err(SelectionFailure::Revision);
        }
        let bytes = initial.encode()?;
        let (installation, ownership, memory) = Self::installation(installation_root, context)?;
        let (root, marker) = protection::with_domain(context, |protection| {
            let _transition = namespace::transition(&installation, protection, context)?;
            for leaf in [DIRECTORY, NAMESPACE_MARKER] {
                match std::fs::symlink_metadata(installation.path.join(leaf)) {
                    Ok(_) => return Err(SelectionFailure::Changed),
                    Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                    Err(cause) => return Err(cause.into()),
                }
            }
            let (stage, identity) = protection::with(context, |owner| {
                let path = staging::unique_path(&installation, ".selection-init")?;
                owner.directory(&path, Mode::CreateNew, context)?;
                let root = Directory::open(&path, false)?;
                owner.file(&path.join(LOCK), context)?.sync_all()?;
                let mut file = owner.file(&path.join(RECORD), context)?;
                file.write_all(&bytes)?;
                file.sync_all()?;
                check(context)?;
                Ok((path, root.identity()?))
            })?;
            let destination = installation.path.join(DIRECTORY);
            protection::with(context, |owner| {
                owner.publish(
                    &stage,
                    &destination,
                    identity,
                    protection::Publication::Directory,
                    context,
                )
            })?;
            let root = Directory::open(&destination, false)?;
            if root.identity()? != identity {
                return Err(super::super::invalid("Published selection directory changed").into());
            }
            let marker = namespace::publish_marker(&installation, &root, protection, context)?;
            Ok((root, marker))
        })
        .map_err(SelectionFailure::from)?;
        Ok(Self {
            root,
            _installation: installation,
            _ownership: ownership,
            _namespace_marker: Some(marker),
            _memory: memory,
        })
    }
}

#[cfg(test)]
mod tests;
