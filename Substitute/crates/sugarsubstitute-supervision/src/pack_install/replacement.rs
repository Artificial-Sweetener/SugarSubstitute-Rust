// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replace only the tracked files of a pack, journaled so it survives a power cut.
//!
//! The journal moves through phases. `backing-up`: nothing in the pack has changed, so
//! recovery only discards the backup. `replacing`: the pack may be half replaced, so recovery
//! restores every backed-up file. `applied`: the new version is in place and waits for its
//! first complete start. `started`: the backend finished a full start and the backup waits
//! for the first page. `unobserved`: the start could not be observed, so the page must also
//! find every node registered. `page-failed`: the page reported a load failure. Any phase
//! but `started` and `unobserved` is restored at the next start unless confirmed first.
//! Untracked files are never touched, except that an untracked file where the new release
//! puts a file is backed up and restored too. The journal lives wherever the caller keeps
//! its transaction folder; the pack folder holds only the pack.

use super::digest_record::FileDigests;
use super::entry::{DIGEST_RECORD_NAME, EntryName, TRACKING_FILE_NAME, parse_entry};
use super::error::{ErrorCode, PackInstallError, Result};
use super::ownership::write_ownership_records;
use crate::durable_storage::pack_tree::{NodeKind, PackTree};
use serde::{Deserialize, Serialize};
use std::{cell::Cell, collections::BTreeSet, io};

const JOURNAL: &str = "journal.json";
const BACKUP_FILES: &str = "files";
const BACKUP_RECORDS: [(&str, &str); 2] =
    [(TRACKING_FILE_NAME, "tracking"), (DIGEST_RECORD_NAME, "digests")];
const JOURNAL_SCHEMA: u32 = 1;
const MAX_JOURNAL_BYTES: usize = 8 * 1024 * 1024;

/// How far a replacement got before the process stopped.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum JournalPhase {
    /// The backup is being written; the pack is untouched.
    BackingUp,
    /// The pack is being replaced.
    Replacing,
    /// The new version is in place and waits for its first complete start.
    Applied,
    /// The backend started fully; the first page decides.
    Started,
    /// The start could not be observed; the first page decides, with every node registered.
    Unobserved,
    /// The first page reported a load failure.
    PageFailed,
}

/// One replacement of an owned pack by an extracted release.
pub(crate) struct ReplacementPlan<'a> {
    pub(crate) target: &'a PackTree,
    pub(crate) source: &'a PackTree,
    pub(crate) old_entries: &'a [EntryName],
    pub(crate) new_entries: &'a [EntryName],
    pub(crate) from_version: &'a str,
    pub(crate) to_version: &'a str,
}

/// A journaled replacement found on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingReplacement {
    pub(crate) phase: JournalPhase,
    pub(crate) from_version: String,
    pub(crate) to_version: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Journal {
    schema: u32,
    pub(super) phase: JournalPhase,
    pub(super) from_version: String,
    pub(super) to_version: String,
    old_entries: Vec<String>,
    new_entries: Vec<String>,
    backed_up: Vec<String>,
    /// Trial endings the host did not witness (power cut, kill) while the version was applied.
    #[serde(default)]
    pub(super) unwitnessed_endings: u32,
}

/// Where a test stops a replacement: the `fail_at`-th pack effect fails (and is rolled back),
/// or the first effect once the journal reaches `halt_in` stops the process like a power cut.
#[derive(Clone, Copy, Default)]
struct Faults {
    fail_at: Option<usize>,
    halt_in: Option<JournalPhase>,
}

/// The journaled replacement transaction kept in one transaction folder.
pub(crate) struct TrackedReplacement {
    root: PackTree,
    faults: Faults,
    steps: Cell<usize>,
    phase: Cell<Option<JournalPhase>>,
    halted: Cell<bool>,
}

impl TrackedReplacement {
    /// Keep the transaction in `root`, a folder outside the pack that the caller owns.
    pub(crate) fn new(root: PackTree) -> Self {
        Self {
            root,
            faults: Faults::default(),
            steps: Cell::new(0),
            phase: Cell::new(None),
            halted: Cell::new(false),
        }
    }

    /// Fail the `fail_at`-th pack effect, or stop at the first effect in `halt_in` as a power
    /// cut would, leaving the journal as it was.
    #[cfg(test)]
    pub(crate) fn with_faults(
        root: PackTree,
        fail_at: Option<usize>,
        halt_in: Option<JournalPhase>,
    ) -> Self {
        Self { faults: Faults { fail_at, halt_in }, ..Self::new(root) }
    }

    /// Return how many pack effects ran so far.
    #[cfg(test)]
    pub(crate) fn effects(&self) -> usize {
        self.steps.get()
    }

    /// Return the journaled replacement, if one is on disk.
    pub(crate) fn pending(&self) -> Result<Option<PendingReplacement>> {
        Ok(self.journal()?.map(|journal| PendingReplacement {
            phase: journal.phase,
            from_version: journal.from_version,
            to_version: journal.to_version,
        }))
    }

    /// Replace the tracked files and leave the journal in the applied phase.
    ///
    /// # Errors
    /// Returns `busy` when an earlier replacement is unsettled. Any other failure restores
    /// the previous files before it is returned.
    pub(crate) fn apply(&self, plan: &ReplacementPlan<'_>) -> Result<()> {
        if self.root.root_kind()? != NodeKind::Missing {
            return Err(PackInstallError::format(
                ErrorCode::Busy,
                "An earlier update has not been settled.",
            ));
        }
        self.root.create()?;
        let backed_up = match self
            .write_journal(plan, JournalPhase::BackingUp, &[])
            .and_then(|()| self.back_up(plan))
        {
            Ok(backed_up) => backed_up,
            Err(cause) => {
                if !self.halted.get() {
                    self.discard_unstarted()?;
                }
                return Err(cause);
            }
        };
        self.write_journal(plan, JournalPhase::Replacing, &backed_up)?;
        let replaced = self
            .replace(plan)
            .and_then(|()| self.write_journal(plan, JournalPhase::Applied, &backed_up));
        if let Err(cause) = replaced {
            if !self.halted.get() {
                tracing::warn!(
                    target: "sugarsubstitute::pack_install",
                    operation = "replace",
                    from = plan.from_version,
                    to = plan.to_version,
                    error = %cause,
                    "The pack replacement failed and is being restored"
                );
                self.roll_back(plan.target)?;
            }
            return Err(cause);
        }
        Ok(())
    }

    /// Record that an applied replacement reached a later point of its trial.
    pub(crate) fn advance(&self, phase: JournalPhase) -> Result<()> {
        let Some(mut journal) = self.journal()? else {
            return Ok(());
        };
        journal.phase = phase;
        self.store(&journal)
    }

    /// Settle a replacement whose new version started and loaded its page.
    pub(crate) fn confirm(&self) -> Result<()> {
        match self.pending()? {
            Some(pending)
                if matches!(
                    pending.phase,
                    JournalPhase::Applied | JournalPhase::Started | JournalPhase::Unobserved
                ) =>
            {
                self.root.remove_all().map_err(PackInstallError::from)
            }
            _ => Ok(()),
        }
    }

    /// Remove a transaction whose backup began but whose pack is untouched.
    pub(crate) fn discard_unstarted(&self) -> Result<()> {
        self.root.remove_all().map_err(PackInstallError::from)
    }

    /// Restore the backed-up files; safe to repeat after an interruption.
    pub(crate) fn roll_back(&self, target: &PackTree) -> Result<()> {
        let Some(journal) = self.journal()? else {
            return Ok(());
        };
        let old_entries = entries(&journal.old_entries)?;
        let new_entries = entries(&journal.new_entries)?;
        let backup = self.root.child(BACKUP_FILES)?;
        for entry in new_entries.iter().filter(|entry| !entry.is_directory()) {
            target.remove_file(entry.path())?;
        }
        for path in &journal.backed_up {
            if let Some((parent, _)) = path.rsplit_once('/') {
                target.create_directories(parent)?;
            }
            target.copy_from(path, &backup, path)?;
        }
        for (name, kept) in BACKUP_RECORDS {
            if self.root.kind(kept)? == NodeKind::File {
                target.copy_from(name, &self.root, kept)?;
            } else {
                target.remove_file(name)?;
            }
        }
        remove_empty_directories(target, &new_entries, &old_entries)?;
        self.root.remove_all().map_err(PackInstallError::from)
    }

    fn step(&self) -> Result<()> {
        let step = self.steps.get() + 1;
        self.steps.set(step);
        if self.faults.halt_in.is_some() && self.faults.halt_in == self.phase.get() {
            self.halted.set(true);
            return Err(
                io::Error::new(io::ErrorKind::Interrupted, "halted like a power cut").into()
            );
        }
        if self.faults.fail_at == Some(step) {
            return Err(io::Error::other("injected pack effect failure").into());
        }
        Ok(())
    }

    fn back_up(&self, plan: &ReplacementPlan<'_>) -> Result<Vec<String>> {
        let new_files: BTreeSet<String> = plan
            .new_entries
            .iter()
            .filter(|entry| !entry.is_directory())
            .map(EntryName::folded)
            .collect();
        let backup = self.root.child(BACKUP_FILES)?;
        backup.create()?;
        let mut seen = BTreeSet::new();
        let mut backed_up = Vec::new();
        let candidates = plan.old_entries.iter().chain(plan.new_entries);
        for entry in candidates.filter(|entry| !entry.is_directory()) {
            if !seen.insert(entry.folded()) {
                continue;
            }
            match plan.target.kind(entry.path())? {
                NodeKind::Directory if new_files.contains(&entry.folded()) => {
                    return Err(PackInstallError::format(
                        ErrorCode::TargetChanged,
                        "A folder sits where the update puts a file.",
                    ));
                }
                NodeKind::File => {
                    self.step()?;
                    if let Some((parent, _)) = entry.path().rsplit_once('/') {
                        backup.create_directories(parent)?;
                    }
                    backup.copy_from(entry.path(), plan.target, entry.path())?;
                    backed_up.push(entry.path().to_owned());
                }
                _ => {}
            }
        }
        for (name, kept) in BACKUP_RECORDS {
            if plan.target.kind(name)? == NodeKind::File {
                self.step()?;
                self.root.copy_from(kept, plan.target, name)?;
            }
        }
        Ok(backed_up)
    }

    fn replace(&self, plan: &ReplacementPlan<'_>) -> Result<()> {
        let new_keys: BTreeSet<String> = plan.new_entries.iter().map(EntryName::folded).collect();
        for entry in plan.old_entries.iter().filter(|entry| !entry.is_directory()) {
            if !new_keys.contains(&entry.folded()) {
                self.step()?;
                plan.target.remove_file(entry.path())?;
            }
        }
        remove_empty_directories(plan.target, plan.old_entries, plan.new_entries)?;
        let mut digests = FileDigests::new();
        for entry in plan.new_entries {
            if entry.is_directory() {
                plan.target.create_directories(entry.path())?;
                continue;
            }
            if let Some((parent, _)) = entry.path().rsplit_once('/') {
                plan.target.create_directories(parent)?;
            }
            self.step()?;
            plan.target.copy_from(entry.path(), plan.source, entry.path())?;
            digests.insert(entry.path().to_owned(), plan.target.sha256(entry.path())?);
        }
        self.step()?;
        write_ownership_records(plan.target, plan.new_entries, plan.to_version, &digests)
    }

    fn write_journal(
        &self,
        plan: &ReplacementPlan<'_>,
        phase: JournalPhase,
        backed_up: &[String],
    ) -> Result<()> {
        let text =
            |entries: &[EntryName]| entries.iter().map(|entry| entry.text().to_owned()).collect();
        self.store(&Journal {
            schema: JOURNAL_SCHEMA,
            phase,
            from_version: plan.from_version.to_owned(),
            to_version: plan.to_version.to_owned(),
            old_entries: text(plan.old_entries),
            new_entries: text(plan.new_entries),
            backed_up: backed_up.to_vec(),
            unwitnessed_endings: 0,
        })
    }

    pub(super) fn store(&self, journal: &Journal) -> Result<()> {
        self.phase.set(Some(journal.phase));
        let bytes = serde_json::to_vec_pretty(journal).map_err(io::Error::other)?;
        self.root.write(JOURNAL, &bytes).map_err(PackInstallError::from)
    }

    pub(super) fn journal(&self) -> Result<Option<Journal>> {
        if self.root.kind(JOURNAL)? == NodeKind::Missing {
            return Ok(None);
        }
        let corrupt = || {
            PackInstallError::format(ErrorCode::StagedCorrupt, "The update journal is unreadable.")
        };
        let bytes = self.root.read(JOURNAL, MAX_JOURNAL_BYTES).map_err(|_| corrupt())?;
        let journal: Journal = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
        if journal.schema != JOURNAL_SCHEMA {
            return Err(corrupt());
        }
        Ok(Some(journal))
    }
}

fn entries(texts: &[String]) -> Result<Vec<EntryName>> {
    texts.iter().map(|text| parse_entry(text)).collect()
}

/// Remove now-empty directories `entries` implied and `keep` does not, deepest first.
fn remove_empty_directories(
    root: &PackTree,
    entries: &[EntryName],
    keep: &[EntryName],
) -> Result<()> {
    let mut kept: BTreeSet<String> =
        keep.iter().filter(|entry| entry.is_directory()).map(EntryName::folded).collect();
    for entry in keep {
        let folded = entry.folded();
        let parts: Vec<&str> = folded.split('/').collect();
        kept.extend((1..parts.len()).map(|depth| parts[..depth].join("/")));
    }
    let mut candidates = BTreeSet::new();
    for entry in entries {
        let parts: Vec<&str> = entry.segments().collect();
        let limit = if entry.is_directory() { parts.len() } else { parts.len() - 1 };
        candidates.extend((1..=limit).map(|depth| parts[..depth].join("/")));
    }
    let mut ordered: Vec<String> = candidates.into_iter().collect();
    ordered.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
    for path in ordered {
        if !kept.contains(&super::entry::fold(&path)) {
            root.remove_empty_directory(&path)?;
        }
    }
    Ok(())
}
