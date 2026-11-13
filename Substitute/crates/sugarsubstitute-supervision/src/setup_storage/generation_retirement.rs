// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Remove application generations nothing can start any more, so updates never fill the disk.
//!
//! One turn runs on an admitted storage worker while the caller holds installation run custody,
//! which excludes the installer, and takes the updater preparation owner without waiting,
//! which excludes every updater; a busy preparation skips the whole turn. Together they exclude
//! every writer that creates or selects a generation, so the record read under them names every
//! generation that may still run.
//!
//! A turn first finishes retirements an earlier turn or a crash interrupted, then keeps the
//! accepted, retained and candidate generations, every generation the launch pointer names,
//! the stable entry's repair source and the running generation. Every other generation folder
//! is retired through [`OwnedRoot`]: renamed to a tombstone, which fails while anything inside
//! is open, then emptied of the files its own package inventory names. Unknown content stays
//! inside the tombstone with one warning, and names that are not generations stay untouched.
mod inventory;
#[cfg(test)]
mod tests;

use super::{
    RunningApplication, SelectionFailure, SelectionStore, check,
    entrypoint::stable,
    invalid,
    launch_pointer::{PointerFailure, read_launch_pointer},
    owned_retirement_root, ownership,
    trust::{PreparationAttempt, try_preparation},
};
use crate::durable_storage::{
    files::Directory,
    owned_retirement::{
        OwnedRoot, RETIREMENT_RESERVATION_BYTES, RetirementError, RetirementOutcome, RetirementUnit,
    },
    protection,
};
use std::{
    collections::BTreeSet,
    fmt, io,
    path::{Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::{
    execution::JobContext,
    installation::{Candidate, GenerationId, InstallationRecord},
};

/// Storage-job reservation for one retirement turn: the retirement workspace, one package
/// inventory at a time and the selection and pointer reads.
pub const GENERATION_RETIREMENT_RESERVATION_BYTES: u64 = RETIREMENT_RESERVATION_BYTES
    + 3 * sugarsubstitute_update::package::MAX_MANIFEST_BYTES as u64
    + 4 * 1024 * 1024;
/// Most entries one turn inspects in the generation folder; more is damage, not updates.
const MAX_RELEASE_ENTRIES: usize = 1024;

/// Generations the stable entry may start, as the installation's launch pointer names them.
///
/// The shape matches the launch pointer owner's `pointed_generations`: the primary and then
/// the fallback, and none when no pointer exists. An unreadable pointer is an error, and the
/// turn then keeps every generation.
pub trait PointedGenerations: Send + Sync {
    /// Read the generations the pointer of the installation at `installation` names.
    /// Runs on the turn's admitted storage worker.
    /// # Errors
    /// `Incompatible`, `Damaged` or `Storage` when the pointer cannot be read.
    fn pointed_generations(
        &self,
        installation: &Path,
        context: &JobContext,
    ) -> Result<Vec<GenerationId>, PointerFailure>;
}

/// The installation's protected launch pointer.
#[derive(Clone, Copy, Debug, Default)]
pub struct LaunchPointerGenerations;

impl PointedGenerations for LaunchPointerGenerations {
    fn pointed_generations(
        &self,
        installation: &Path,
        context: &JobContext,
    ) -> Result<Vec<GenerationId>, PointerFailure> {
        match read_launch_pointer(installation, context) {
            Ok(pointer) => Ok(std::iter::once(pointer.primary())
                .chain(pointer.fallback())
                .map(|target| target.generation().clone())
                .collect()),
            Err(PointerFailure::Missing) => Ok(Vec::new()),
            Err(cause) => Err(cause),
        }
    }
}

/// Frees disk space held by generations nothing can start, for a path that has run short of it.
///
/// A disk-shortage path calls this before reporting that space is insufficient, and reads
/// [`GenerationReport::freed_bytes`] to decide whether to try again. It never waits: a turn
/// that overlaps a preparation is skipped, so a preparation that runs short must release
/// preparation custody before asking for space.
pub trait GenerationReclaim: Send + Sync {
    /// Run one retirement turn now. Runs on an admitted storage worker reserving
    /// [`GENERATION_RETIREMENT_RESERVATION_BYTES`].
    /// # Errors
    /// As [`retire_generations`].
    fn reclaim(&self, context: &JobContext) -> Result<GenerationTurn, GenerationRetirementError>;
}

/// The coordinator's reclaim hook, bound to the run custody its retained generation holds.
pub struct RunningGenerationReclaim {
    application: Arc<RunningApplication>,
    pointer: Arc<dyn PointedGenerations>,
}

impl RunningGenerationReclaim {
    /// Reclaim under `application`'s run custody, keeping what `pointer` names.
    pub fn new(application: Arc<RunningApplication>, pointer: Arc<dyn PointedGenerations>) -> Self {
        Self { application, pointer }
    }
}

impl GenerationReclaim for RunningGenerationReclaim {
    fn reclaim(&self, context: &JobContext) -> Result<GenerationTurn, GenerationRetirementError> {
        retire_generations(&self.application, self.pointer.as_ref(), context)
    }
}

/// How one retirement turn ended.
#[derive(Debug)]
pub enum GenerationTurn {
    /// The turn inspected every generation.
    Finished(GenerationReport),
    /// A preparation was running, so nothing was inspected or changed. A later turn retries.
    PreparationBusy,
    /// The launch pointer could not be read, so every generation was kept.
    PointerUnreadable(PointerFailure),
}

/// What one finished turn removed and what it left.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GenerationReport {
    /// Generations removed completely, including interrupted retirements finished now.
    pub removed: u64,
    /// Generations left untouched because something holds them open; a later turn retries.
    pub in_use: u64,
    /// Retired generations whose folder remains for preserved or still-open content.
    pub retained: u64,
    /// Unknown files, folders and links kept inside retired generations.
    pub preserved: u64,
    /// Entries of the generation folder that are not generations, left untouched.
    pub unknown: u64,
    /// Bytes of the inventoried files deleted.
    pub freed_bytes: u64,
}

/// Why a turn could not run or finish. Work already committed resumes on a later turn.
#[derive(Debug)]
pub enum GenerationRetirementError {
    /// The job was cancelled or reached its deadline.
    Stopped(io::Error),
    /// The selection record could not be read; nothing was retired.
    Selection(SelectionFailure),
    /// A retirement failed; its typed cause is kept.
    Retirement(RetirementError),
    /// Storage failed outside a retirement; the native cause is kept.
    Storage(io::Error),
}

impl fmt::Display for GenerationRetirementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Stopped(_) => "Old version cleanup stopped before it finished",
            Self::Selection(_) => "Old version cleanup could not read the installation selection",
            Self::Retirement(_) => "Old version cleanup could not remove a version",
            Self::Storage(_) => "Old version cleanup storage failed",
        })
    }
}

impl std::error::Error for GenerationRetirementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Stopped(cause) | Self::Storage(cause) => Some(cause),
            Self::Selection(cause) => Some(cause),
            Self::Retirement(cause) => Some(cause),
        }
    }
}

impl From<io::Error> for GenerationRetirementError {
    fn from(cause: io::Error) -> Self {
        match cause.kind() {
            io::ErrorKind::Interrupted | io::ErrorKind::TimedOut => Self::Stopped(cause),
            _ => Self::Storage(cause),
        }
    }
}

impl From<RetirementError> for GenerationRetirementError {
    fn from(cause: RetirementError) -> Self {
        match cause {
            RetirementError::Stopped(cause) => Self::Stopped(cause),
            cause => Self::Retirement(cause),
        }
    }
}

impl From<SelectionFailure> for GenerationRetirementError {
    fn from(cause: SelectionFailure) -> Self {
        Self::Selection(cause)
    }
}

/// Retire every generation of `application`'s installation that nothing can start any more.
///
/// `application` is the coordinator's retained generation; its run custody is the turn's
/// first exclusion. Call after the coordinator retains its generation and, for a trial, after
/// the trial is accepted. Runs on an admitted storage worker reserving
/// [`GENERATION_RETIREMENT_RESERVATION_BYTES`]; cancellation stops between objects and leaves
/// committed tombstones for the next turn.
/// # Errors
/// `Stopped` on cancellation or deadline, `Selection` when the record cannot be read,
/// `Retirement` and `Storage` with their causes. A failed turn never removes a kept generation.
pub fn retire_generations(
    application: &RunningApplication,
    pointer: &dyn PointedGenerations,
    context: &JobContext,
) -> Result<GenerationTurn, GenerationRetirementError> {
    let installation = installation_of(application.directory())?;
    retire_held(&installation, Some(&application.generation().id), pointer, context)
}

/// The installation folder above a running generation folder `<installation>\releases\<id>`.
fn installation_of(generation: &Path) -> io::Result<PathBuf> {
    generation
        .parent()
        .filter(|releases| {
            releases
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("releases"))
        })
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| invalid("Running generation is not inside an installation"))
}

/// One turn for the installation at `installation`. The caller holds its run custody.
/// `running` names the generation this process runs, which is kept whatever the record says.
pub(in crate::setup_storage) fn retire_held(
    installation: &Path,
    running: Option<&GenerationId>,
    pointer: &dyn PointedGenerations,
    context: &JobContext,
) -> Result<GenerationTurn, GenerationRetirementError> {
    check(context)?;
    let _work = context
        .memory()
        .claim(3 * sugarsubstitute_update::package::MAX_MANIFEST_BYTES as u64)
        .map_err(io::Error::other)?;
    let pinned = Directory::open(installation, false)?;
    let _preparation = match try_preparation(&pinned, context)? {
        PreparationAttempt::Held(file) => Some(file),
        PreparationAttempt::Absent => None,
        PreparationAttempt::Busy => return Ok(GenerationTurn::PreparationBusy),
    };
    let releases_path = installation.join("releases");
    let root = owned_retirement_root(installation, &releases_path, context)?;
    let mut report = resume(&root, context)?;
    let Some(record) = SelectionStore::open(installation, context)?.load(context)? else {
        return Err(SelectionFailure::Missing.into());
    };
    let pointed = match pointer.pointed_generations(installation, context) {
        Ok(pointed) => pointed,
        Err(cause) => {
            tracing::warn!(
                operation = "generation_retirement_pointer_unreadable",
                cause = %cause,
                "Old version cleanup kept every version because the launch pointer is unreadable"
            );
            return Ok(GenerationTurn::PointerUnreadable(cause));
        }
    };
    let source = protection::with(context, |protection| {
        let state = Directory::open(&ownership::state_path(&pinned), false)?;
        stable::recovery_source(&state, protection, context)
    })?;
    let kept = keep_set(&record, pointed, source, running);
    let releases = Directory::open(&releases_path, false)?;
    let (names, other) = release_names(&releases, context)?;
    report.unknown += other;
    for name in names {
        check(context)?;
        let unit = match classify(&name, &kept) {
            Entry::Kept | Entry::Tombstone => continue,
            Entry::Unknown => {
                report.unknown += 1;
                continue;
            }
            Entry::Generation => {
                RetirementUnit::Tree(inventory::of_generation(&releases, &name, context)?)
            }
            Entry::Initializing => RetirementUnit::Tree(inventory::initializing()?),
        };
        let retired = root.retire(&name, &unit, context)?;
        report.freed_bytes += retired.freed_bytes;
        match retired.outcome {
            RetirementOutcome::Removed => report.removed += 1,
            RetirementOutcome::Busy => report.in_use += 1,
            RetirementOutcome::Retained { preserved, .. } => {
                report.retained += 1;
                report.preserved += preserved;
            }
            RetirementOutcome::Absent => {}
        }
    }
    if report.unknown > 0 {
        tracing::warn!(
            operation = "generation_retirement_unknown_entries",
            unknown = report.unknown,
            "Old version cleanup left entries it does not recognize"
        );
    }
    tracing::info!(
        operation = "generation_retirement_finished",
        removed = report.removed,
        in_use = report.in_use,
        retained = report.retained,
        freed_bytes = report.freed_bytes,
        "Old version cleanup finished"
    );
    Ok(GenerationTurn::Finished(report))
}

/// Finish every interrupted retirement from the tombstone's own package records.
fn resume(root: &OwnedRoot, context: &JobContext) -> Result<GenerationReport, RetirementError> {
    let resumed =
        root.resume(&mut |tombstone| inventory::of_tombstone(tombstone, context), context)?;
    Ok(GenerationReport {
        removed: resumed.removed,
        retained: resumed.retained,
        preserved: resumed.preserved,
        freed_bytes: resumed.freed_bytes,
        ..GenerationReport::default()
    })
}

/// Every generation that may still run or be started.
fn keep_set(
    record: &InstallationRecord,
    pointed: Vec<GenerationId>,
    source: Option<GenerationId>,
    running: Option<&GenerationId>,
) -> BTreeSet<String> {
    record
        .accepted()
        .into_iter()
        .chain(record.retained())
        .chain(record.candidate().map(Candidate::generation))
        .map(|generation| generation.id.clone())
        .chain(pointed)
        .chain(source)
        .chain(running.cloned())
        .map(|id| id.as_str().to_ascii_lowercase())
        .collect()
}

/// What one entry of the generation folder is to this owner.
enum Entry {
    /// A generation that may still run.
    Kept,
    /// A generation nothing can start.
    Generation,
    /// An interrupted creation of a generation folder, never published.
    Initializing,
    /// A retirement tombstone, which resuming handles.
    Tombstone,
    /// Anything else, which belongs to no owner here.
    Unknown,
}

fn classify(name: &str, kept: &BTreeSet<String>) -> Entry {
    if GenerationId::validate(name).is_ok() {
        // Windows compares names without case, so a kept identity keeps every spelling.
        if kept.contains(&name.to_ascii_lowercase()) { Entry::Kept } else { Entry::Generation }
    } else if inventory::is_initializing(name) {
        Entry::Initializing
    } else if name.get(..9).is_some_and(|prefix| prefix.eq_ignore_ascii_case(".retired-")) {
        Entry::Tombstone
    } else {
        Entry::Unknown
    }
}

/// The Unicode names of the generation folder's entries in name order, bounded, and how many
/// other names it holds, which no owner here wrote.
fn release_names(releases: &Directory, context: &JobContext) -> io::Result<(Vec<String>, u64)> {
    let mut names = Vec::new();
    let mut other = 0;
    for entry in releases.entries(context)? {
        check(context)?;
        let entry = entry?;
        if names.len() as u64 + other == MAX_RELEASE_ENTRIES as u64 {
            return Err(invalid("The generation folder holds more entries than cleanup inspects"));
        }
        match entry.file_name().into_string() {
            Ok(name) => names.push(name),
            Err(_) => other += 1,
        }
    }
    names.sort();
    Ok((names, other))
}
