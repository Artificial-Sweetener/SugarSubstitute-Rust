// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the Comfy Registry install format of the SugarCubes node pack on the host side.
//!
//! SugarCubes' Python package `sugarcubes.backend.pack_install` is the other implementation
//! of the same format. Both read SugarCubes' registry-install vector file in place, and a repository check fails unless every vector group is driven by both, so
//! the two never drift. A folder installed here is indistinguishable from a registry install:
//! `.tracking` lists exactly the owned entries and `.tracking-digests.json` binds their
//! digests to those bytes, so ComfyUI Manager and SugarCubes both recognise it.
//!
//! [`PackFolder`] addresses the installed pack, [`PackTransaction`] the host's own update
//! transaction, which lives in the host's storage, never inside the pack. All file effects
//! run in work the execution owner admitted; nothing here blocks a UI thread.

mod archive;
mod digest_record;
mod entry;
mod error;
mod identity;
mod ownership;
mod replacement;
mod selection;
mod sources;
mod tracking;
mod trial;
#[cfg(test)]
mod vector_tests;

pub use archive::{ArchiveLimits, ExtractedArchive, list_registry_archive};
pub use digest_record::{
    DIGEST_RECORD_VERSION, FileDigests, MAX_DIGEST_RECORD_BYTES, parse_digest_record,
    render_digest_record,
};
pub use entry::{
    DIGEST_RECORD_NAME, EntryName, TRACKING_FILE_NAME, parse_entry, validate_entry_set,
};
pub use error::{ErrorCode, PackInstallError, Result};
pub use identity::{MAX_PYPROJECT_BYTES, PackIdentity, ReleaseVersion};
pub use ownership::{InstalledDigests, NotOwnedReason, changed_tracked_files};
pub use replacement::JournalPhase;
pub use selection::{ReleaseSources, SourceFailure, UpdateDecision, newest_offer};
pub use sources::{
    MAX_ARCHIVE_BYTES, MetadataError, ReleaseOffer, ReleaseSourceConfig, ReleaseSourceKind,
    github_exact, github_latest, registry_exact, registry_latest,
};
pub use tracking::{
    MAX_TRACKED_ENTRIES, MAX_TRACKING_BYTES, TRACKING_FORMAT_VERSION, parse_tracking,
    render_tracking, tracking_bytes,
};
pub use trial::{RestoreReason, Restored, TrialEnding, TrialEvent, UNWITNESSED_ENDINGS_LIMIT};

use crate::durable_storage::pack_tree::PackTree;
use ownership::{PackOwnership, inspect_pack_folder, installed_file_digests, read_identity};
use replacement::{ReplacementPlan, TrackedReplacement};
use std::{io, path::Path};

/// What an installed pack folder is to this host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ownership {
    /// A registry install the host may replace.
    Owned(PackIdentity),
    /// A folder the host leaves alone, and why.
    NotOwned(NotOwnedReason),
}

/// The pack folder ComfyUI loads, addressed as ComfyUI reaches it (never resolved, which
/// would hide a link).
pub struct PackFolder {
    tree: PackTree,
}

impl PackFolder {
    /// Address one absolute pack folder.
    ///
    /// # Errors
    /// Refuses a relative or non-local path.
    pub fn at(path: &Path) -> io::Result<Self> {
        Ok(Self { tree: PackTree::at(path)? })
    }

    /// Classify the folder without changing it.
    ///
    /// # Errors
    /// Returns a filesystem failure; every format problem is a [`NotOwnedReason`].
    pub fn ownership(&self) -> Result<Ownership> {
        Ok(match inspect_pack_folder(&self.tree)? {
            PackOwnership::Owned(pack) => Ownership::Owned(pack.identity),
            PackOwnership::NotOwned(reason) => Ownership::NotOwned(reason),
        })
    }

    /// Decide what one update check installs, reading releases through `sources`.
    ///
    /// # Errors
    /// Returns a filesystem failure; source failures are part of the decision.
    pub fn plan_update(
        &self,
        config: &ReleaseSourceConfig,
        sources: &dyn ReleaseSources,
    ) -> Result<UpdateDecision> {
        selection::plan_update(&self.tree, config, sources)
    }
}

/// The host's journaled update transaction for one pack folder.
pub struct PackTransaction {
    replacement: TrackedReplacement,
    staging: PackTree,
}

impl PackTransaction {
    /// Keep the transaction in `root`, a folder in the host's own storage.
    ///
    /// # Errors
    /// Refuses a relative or non-local path.
    pub fn at(root: &Path) -> io::Result<Self> {
        let parent =
            root.parent().ok_or_else(|| io::Error::other("Transaction root has no parent"))?;
        let name = root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::other("Transaction root has no name"))?;
        Ok(Self {
            replacement: TrackedReplacement::new(PackTree::at(root)?),
            staging: PackTree::at(&parent.join(format!("{name}.staging")))?,
        })
    }

    /// Return the phase and versions of an unsettled replacement, if any.
    ///
    /// # Errors
    /// Returns `staged-corrupt` for an unreadable journal.
    pub fn pending(&self) -> Result<Option<(JournalPhase, String, String)>> {
        Ok(self
            .replacement
            .pending()?
            .map(|pending| (pending.phase, pending.from_version, pending.to_version)))
    }

    /// Install one downloaded release over the unchanged tracked files of `folder`.
    ///
    /// The archive must be the pack named `pack_id` at `offer`'s version, and the tracked
    /// files must still have the digests `installed` the plan was made against.
    ///
    /// # Errors
    /// Returns the shared refusal for an invalid archive or a changed folder, or `busy` when
    /// an earlier replacement is unsettled; a failed replacement is restored first.
    pub fn install(
        &self,
        folder: &PackFolder,
        archive: &[u8],
        pack_id: &str,
        offer: &ReleaseOffer,
        installed: &InstalledDigests,
    ) -> Result<()> {
        self.staging.remove_all()?;
        let installed_now = self.install_staged(folder, archive, pack_id, offer, installed);
        let cleaned = self.staging.remove_all();
        installed_now?;
        cleaned.map_err(PackInstallError::from)
    }

    fn install_staged(
        &self,
        folder: &PackFolder,
        archive: &[u8],
        pack_id: &str,
        offer: &ReleaseOffer,
        installed: &InstalledDigests,
    ) -> Result<()> {
        let extracted =
            archive::extract_registry_archive(archive, &self.staging, ArchiveLimits::default())?;
        let identity = read_identity(&self.staging)?;
        if !identity.is_pack(pack_id) || identity.version != offer.version {
            return Err(PackInstallError::format(
                ErrorCode::PackIdentity,
                "The archive is not the expected release.",
            ));
        }
        let PackOwnership::Owned(pack) = inspect_pack_folder(&folder.tree)? else {
            return Err(PackInstallError::format(
                ErrorCode::TargetChanged,
                "The pack folder changed.",
            ));
        };
        if &installed_file_digests(&folder.tree, &pack)? != installed {
            return Err(PackInstallError::format(
                ErrorCode::TargetChanged,
                "The pack folder changed.",
            ));
        }
        self.replacement.apply(&ReplacementPlan {
            target: &folder.tree,
            source: &self.staging,
            old_entries: &pack.entries,
            new_entries: &extracted.entries,
            from_version: &pack.identity.version.to_string(),
            to_version: &offer.version.to_string(),
        })
    }

    /// Record one observation of the just-installed `running_version`.
    ///
    /// # Errors
    /// Returns a journal or filesystem failure.
    pub fn observe(&self, running_version: &str, event: TrialEvent) -> Result<()> {
        self.replacement.observe(running_version, event)
    }

    /// Settle the judgement the previous run left open, restoring `folder` when the new
    /// version failed.
    ///
    /// # Errors
    /// Returns a journal or filesystem failure; the journal stays for the next attempt.
    pub fn settle(&self, folder: &PackFolder, ending: TrialEnding) -> Result<Option<Restored>> {
        self.replacement.settle(&folder.tree, ending)
    }
}
