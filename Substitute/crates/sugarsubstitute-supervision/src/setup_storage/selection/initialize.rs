// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish complete revision-zero state before enabling its durable continuity marker.
use super::*;

/// Prefix of the folder a first selection record is staged in.
const INITIAL_STAGE: &str = ".selection-init";

impl SelectionStore {
    /// Initialize a previously absent selection with one atomic directory publication.
    /// The initial record must be revision zero. Staging is never read as authority or reused.
    /// If publication completed before interruption, reopen the store instead of initializing it.
    /// # Errors
    /// Rejects every occupied namespace or continuity marker, including incomplete state.
    /// Preserves unknown files and any already published selection; removes abandoned staging.
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
        let state = super::super::ownership::state(&installation)?;
        let (root, marker) = protection::with_domain(context, |protection| {
            let _transition = namespace::transition(&installation, protection, context)?;
            for leaf in [DIRECTORY, NAMESPACE_MARKER] {
                match crate::durable_storage::files::inspect(state.path().join(leaf), context) {
                    Ok(_) => return Err(SelectionFailure::Changed),
                    Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                    Err(cause) => return Err(cause.into()),
                }
            }
            // Initialization runs only under this transition custody; earlier attempts are dead.
            protection::with(context, |owner| {
                staging::reclaim(&state, INITIAL_STAGE, &[LOCK, RECORD], owner, context)
            })?;
            let destination = state.path().join(DIRECTORY);
            let mut write = write_port::begin(UpdateFile::Selection, &destination.join(RECORD));
            let (stage, identity) = protection::with(context, |owner| {
                let path = staging::unique_path(&state, INITIAL_STAGE)?;
                owner.directory(&path, Mode::CreateNew, context)?;
                let root = Directory::open(&path, false)?;
                files::sync(&owner.file(&path.join(LOCK), context)?, context)?;
                let mut file = owner.file(&path.join(RECORD), context)?;
                write.bytes(&mut file, &bytes)?;
                files::sync(&file, context)?;
                check(context)?;
                Ok((path, root.identity()?))
            })?;
            protection::with(context, |owner| {
                write.publish(|| {
                    owner.publish(
                        &stage,
                        &destination,
                        identity,
                        protection::Publication::Directory,
                        context,
                    )
                })
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
            clock: StrikeClock::System,
        })
    }
}

#[cfg(test)]
mod tests;
