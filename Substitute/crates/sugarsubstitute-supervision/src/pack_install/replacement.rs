// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replace only the tracked files of a pack, journaled so it survives a power cut.
//!
//! The journal moves through phases. `backing-up`: nothing in the pack has changed, so
//! recovery only discards the backup. `replacing`: the pack may be half replaced, so recovery
//! restores every backed-up file. `applied`: the new version is in place and waits for its
//! first complete start. `started`: the backend finished a full start and the backup waits
//! for the first page. `unobserved`: the start could not be observed, so the page must also
//! find every node registered. `page-failed`: the page reported a load failure.
//! `page-unconfirmed`: a page did not load for a cause the host could not attribute, and no
//! later page of that run has loaded yet. Any phase but `started` and `unobserved` is
//! restored at the next start unless confirmed first; those two are restored once three
//! starts in a row end without their page deciding.
//! Untracked files are never touched, except that an untracked file where the new release
//! puts a file is backed up and restored too. The journal lives wherever the caller keeps
//! its transaction folder; the pack folder holds only the pack.

use super::digest_record::FileDigests;
use super::entry::{DIGEST_RECORD_NAME, EntryName, TRACKING_FILE_NAME, parse_entry};
use super::error::{ErrorCode, PackInstallError, Result};
use super::ownership::write_ownership_records;
use crate::durable_storage::pack_tree::{NodeKind, PackTree, Transfer};
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    collections::BTreeSet,
    io,
    time::{Duration, Instant},
};

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
    /// A page did not load for a cause the host could not attribute; a later page decides.
    PageUnconfirmed,
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
    /// Trial endings the host did not witness (power cut, kill) before the page decided.
    #[serde(default)]
    pub(super) unwitnessed_endings: u32,
}

/// Where a test stops a replacement: the `fail_at`-th pack effect fails (and is rolled back),
/// or the first effect once the journal reaches `halt_in` stops the process like a power cut.
#[derive(Clone, Copy, Default)]
struct Faults {
    fail_at: Option<usize>,
    halt_in: Option<JournalPhase>,
    halt_at: Option<usize>,
}

/// How long each phase of putting the previous files back took, and how they came back.
#[derive(Clone, Debug, Default)]
pub(crate) struct RestoreTimings {
    pub(crate) remove_new: Duration,
    pub(crate) restore_files: Duration,
    pub(crate) flush: Duration,
    pub(crate) restore_records: Duration,
    pub(crate) remove_directories: Duration,
    pub(crate) discard_transaction: Duration,
    /// Files renamed back from the backup on the same drive.
    pub(crate) moved: usize,
    /// Files copied back because the backup is on another drive.
    pub(crate) copied: usize,
}

impl RestoreTimings {
    /// The timings in milliseconds, for a log line or a measurement record.
    pub(crate) fn to_json(&self) -> serde_json::Value {
        let ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
        serde_json::json!({
            "removeNewMs": ms(self.remove_new),
            "restoreFilesMs": ms(self.restore_files),
            "flushMs": ms(self.flush),
            "restoreRecordsMs": ms(self.restore_records),
            "removeDirectoriesMs": ms(self.remove_directories),
            "discardTransactionMs": ms(self.discard_transaction),
            "moved": self.moved,
            "copied": self.copied,
        })
    }
}

/// How long each phase of putting a new version's files in place took.
#[derive(Clone, Debug, Default)]
pub(crate) struct InstallTimings {
    /// Setting the files the update changes aside in the backup.
    pub(crate) back_up: Duration,
    /// Making the extracted files' bytes durable, all together.
    pub(crate) flush_staged: Duration,
    /// Putting the new files in place.
    pub(crate) place_new: Duration,
    /// Writing the ownership and digest records.
    pub(crate) records: Duration,
    /// Making the new names durable, once.
    pub(crate) flush_names: Duration,
    /// Files moved by name on the same drive.
    pub(crate) moved: usize,
    /// Files copied because they were on another drive.
    pub(crate) copied: usize,
}

impl InstallTimings {
    /// The timings in milliseconds, for a log line or a measurement record.
    pub(crate) fn to_json(&self) -> serde_json::Value {
        let ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
        serde_json::json!({
            "backUpMs": ms(self.back_up),
            "flushStagedMs": ms(self.flush_staged),
            "placeNewMs": ms(self.place_new),
            "recordsMs": ms(self.records),
            "flushNamesMs": ms(self.flush_names),
            "moved": self.moved,
            "copied": self.copied,
        })
    }
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
        Self { faults: Faults { fail_at, halt_in, halt_at: None }, ..Self::new(root) }
    }

    /// Stop before the `step`-th pack effect as a power cut would, leaving everything as it is.
    #[cfg(test)]
    pub(crate) fn cut_before(root: PackTree, step: usize) -> Self {
        Self { faults: Faults { halt_at: Some(step), ..Faults::default() }, ..Self::new(root) }
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
        let timings = self.apply_timed(plan)?;
        tracing::info!(
            target: "sugarsubstitute::pack_install",
            operation = "install_phases",
            timings = %timings.to_json(),
            "The new SugarCubes files are in place"
        );
        Ok(())
    }

    /// [`Self::apply`], returning how long each phase took.
    ///
    /// The files the update changes are moved aside into the backup by name, after the two
    /// ownership records are copied there durably and the journal names every file it will
    /// move. The extracted files' bytes are flushed together, then moved into place by name,
    /// and one flush makes all the new names durable before the journal says the version is
    /// applied. A cut at any point leaves a journal the restore finishes from, or no journal
    /// and an untouched pack.
    pub(crate) fn apply_timed(&self, plan: &ReplacementPlan<'_>) -> Result<InstallTimings> {
        let mut timings = InstallTimings::default();
        if self.root.root_kind()? != NodeKind::Missing {
            if self.journal()?.is_some() {
                return Err(PackInstallError::format(
                    ErrorCode::Busy,
                    "An earlier update has not been settled.",
                ));
            }
            // A transaction cut off before its journal existed never touched the pack.
            self.discard_unstarted()?;
        }
        self.root.create()?;
        let phase = Instant::now();
        let planned = match self.plan_backup(plan).and_then(|planned| {
            self.keep_records(plan)?;
            self.write_journal(plan, JournalPhase::BackingUp, &planned)?;
            self.back_up(plan, &planned, &mut timings)?;
            Ok(planned)
        }) {
            Ok(planned) => planned,
            Err(cause) => {
                if !self.halted.get() {
                    self.abandon(plan.target)?;
                }
                return Err(cause);
            }
        };
        timings.back_up = phase.elapsed();
        let replaced = self
            .write_journal(plan, JournalPhase::Replacing, &planned)
            .and_then(|()| self.replace(plan, &mut timings))
            .and_then(|()| self.write_journal(plan, JournalPhase::Applied, &planned));
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
        Ok(timings)
    }

    /// Undo a replacement that failed before its new files were placed: restore what was
    /// moved aside when the journal exists, otherwise the pack was never touched.
    fn abandon(&self, target: &PackTree) -> Result<()> {
        if self.journal()?.is_some() { self.roll_back(target) } else { self.discard_unstarted() }
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
                    JournalPhase::Applied
                        | JournalPhase::Started
                        | JournalPhase::Unobserved
                        | JournalPhase::PageUnconfirmed
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
        if let Some(timings) = self.roll_back_timed(target)? {
            tracing::info!(
                target: "sugarsubstitute::pack_install",
                operation = "restore_phases",
                timings = %timings.to_json(),
                "The previous SugarCubes files are back in place"
            );
        }
        Ok(())
    }

    /// [`Self::roll_back`], returning how long each phase took when a journal was found.
    ///
    /// The backup lives in the host's storage beside the pack, so each backed-up file is
    /// renamed back rather than copied: its bytes reached the disk when it was backed up, and
    /// one flush makes all the renames durable before the transaction is removed. A file
    /// already moved back before an interruption is no longer in the backup and is left as it
    /// is, and only files the new version added are removed, so a repeat after a power cut
    /// finishes the same restore.
    pub(crate) fn roll_back_timed(&self, target: &PackTree) -> Result<Option<RestoreTimings>> {
        let Some(journal) = self.journal()? else {
            return Ok(None);
        };
        let mut timings = RestoreTimings::default();
        let old_entries = entries(&journal.old_entries)?;
        let new_entries = entries(&journal.new_entries)?;
        let backed_up = entries(&journal.backed_up)?;
        let restored: BTreeSet<String> = backed_up.iter().map(EntryName::folded).collect();
        let backup = self.root.child(BACKUP_FILES)?;
        let phase = Instant::now();
        for entry in new_entries
            .iter()
            .filter(|entry| !entry.is_directory() && !restored.contains(&entry.folded()))
        {
            self.step()?;
            target.remove_file(entry.path())?;
        }
        timings.remove_new = phase.elapsed();
        let phase = Instant::now();
        for entry in &backed_up {
            if backup.kind(entry.path())? != NodeKind::File {
                continue;
            }
            self.step()?;
            if let Some((parent, _)) = entry.path().rsplit_once('/') {
                target.create_directories(parent)?;
            }
            match target.move_from(entry.path(), &backup, entry.path())? {
                Transfer::Moved => timings.moved += 1,
                Transfer::Copied => timings.copied += 1,
            }
        }
        timings.restore_files = phase.elapsed();
        let phase = Instant::now();
        for (name, kept) in BACKUP_RECORDS {
            self.step()?;
            if self.root.kind(kept)? == NodeKind::File {
                target.copy_from(name, &self.root, kept)?;
            } else {
                target.remove_file(name)?;
            }
        }
        timings.restore_records = phase.elapsed();
        let phase = Instant::now();
        remove_empty_directories(target, &new_entries, &old_entries)?;
        timings.remove_directories = phase.elapsed();
        let phase = Instant::now();
        target.flush_names()?;
        timings.flush = phase.elapsed();
        let phase = Instant::now();
        self.root.remove_all().map_err(PackInstallError::from)?;
        timings.discard_transaction = phase.elapsed();
        Ok(Some(timings))
    }
    fn step(&self) -> Result<()> {
        let step = self.steps.get() + 1;
        self.steps.set(step);
        if (self.faults.halt_in.is_some() && self.faults.halt_in == self.phase.get())
            || self.faults.halt_at == Some(step)
        {
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

    /// Every file the update may change that is in the pack now: the backup's contents, named
    /// in the journal before anything moves.
    fn plan_backup(&self, plan: &ReplacementPlan<'_>) -> Result<Vec<String>> {
        let new_files: BTreeSet<String> = plan
            .new_entries
            .iter()
            .filter(|entry| !entry.is_directory())
            .map(EntryName::folded)
            .collect();
        let mut seen = BTreeSet::new();
        let mut planned = Vec::new();
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
                NodeKind::File => planned.push(entry.path().to_owned()),
                _ => {}
            }
        }
        Ok(planned)
    }

    /// Copy the ownership records into the transaction durably, before anything moves.
    fn keep_records(&self, plan: &ReplacementPlan<'_>) -> Result<()> {
        for (name, kept) in BACKUP_RECORDS {
            if plan.target.kind(name)? == NodeKind::File {
                self.step()?;
                self.root.copy_from(kept, plan.target, name)?;
            }
        }
        Ok(())
    }

    /// Move each planned file aside by name; their bytes are already on the disk.
    fn back_up(
        &self,
        plan: &ReplacementPlan<'_>,
        planned: &[String],
        timings: &mut InstallTimings,
    ) -> Result<()> {
        let backup = self.root.child(BACKUP_FILES)?;
        backup.create()?;
        for path in planned {
            if plan.target.kind(path)? != NodeKind::File {
                continue;
            }
            self.step()?;
            if let Some((parent, _)) = path.rsplit_once('/') {
                backup.create_directories(parent)?;
            }
            match backup.move_from(path, plan.target, path)? {
                Transfer::Moved => timings.moved += 1,
                Transfer::Copied => timings.copied += 1,
            }
        }
        Ok(())
    }

    fn replace(&self, plan: &ReplacementPlan<'_>, timings: &mut InstallTimings) -> Result<()> {
        let phase = Instant::now();
        for entry in plan.new_entries.iter().filter(|entry| !entry.is_directory()) {
            self.step()?;
            plan.source.flush_file(entry.path())?;
        }
        timings.flush_staged = phase.elapsed();
        let phase = Instant::now();
        // Every file of the old version is in the backup now, so only its folders remain.
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
            let digest = plan.source.sha256(entry.path())?;
            match plan.target.move_from(entry.path(), plan.source, entry.path())? {
                Transfer::Moved => timings.moved += 1,
                Transfer::Copied => timings.copied += 1,
            }
            digests.insert(entry.path().to_owned(), digest);
        }
        timings.place_new = phase.elapsed();
        let phase = Instant::now();
        self.step()?;
        write_ownership_records(plan.target, plan.new_entries, plan.to_version, &digests)?;
        timings.records = phase.elapsed();
        let phase = Instant::now();
        plan.target.flush_names()?;
        self.root.flush_names()?;
        timings.flush_names = phase.elapsed();
        Ok(())
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
