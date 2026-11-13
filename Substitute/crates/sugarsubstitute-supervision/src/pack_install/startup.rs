// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Update SugarCubes during the host's splash, before the managed ComfyUI backend loads it.
//!
//! One run settles the judgement the previous start left open, then (unless an update is
//! still being judged) asks both release sources for the newest version, downloads it into
//! the host's own storage, verifies it and replaces only the tracked files, keeping a backup.
//! Every failure leaves a working pack: the installed version, or the previous one restored.
//! A run never blocks the start; its [`StartupOutcome`] says what one plain line to show.
//!
//! While it changes the folder, the run holds the same lock SugarCubes' own updater takes
//! (`.sugarcubes/updates/.lock` inside the pack), so the two implementations of the format
//! never change one folder at once. A version that was restored is not installed again;
//! only a newer one is. All of this is blocking file and network work, run on a worker the
//! execution owner admitted; `proceed` reports that worker's cancellation and deadline.

use super::digest_record::is_sha256;
use super::error::{ErrorCode, PackInstallError, Result};
use super::identity::ReleaseVersion;
use super::ownership::NotOwnedReason;
use super::selection::{ReleaseSources, SourceFailure, UpdateDecision, newest_offer};
use super::sources::{ReleaseOffer, ReleaseSourceConfig, ReleaseSourceKind};
use super::trial::{Restored, TrialEnding, TrialEvent};
use super::{JournalPhase, Ownership, PackFolder, PackTransaction};
use crate::durable_storage::pack_tree::PackLock;
use crate::durable_storage::pack_tree::{NodeKind, PackTree};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Seek},
    path::Path,
};
use sugarsubstitute_contracts::update::pack::{PackOutcome, PackUpdateReport};

/// Where an installed SugarCubes describes its own release sources.
pub const RELEASE_SOURCE_FILE: &str = "sugarcubes/backend/self_update/release_source.json";
/// The largest release-source description read.
pub const MAX_RELEASE_SOURCE_BYTES: usize = 64 * 1024;
/// The lock SugarCubes' own updater holds while it changes the folder.
pub const UPDATE_LOCK_FILE: &str = ".sugarcubes/updates/.lock";
/// Memory one run may use beyond the HTTP owner's own admission: two metadata replies,
/// archive and extraction buffers, and the digest maps of a full release.
pub const STARTUP_UPDATE_RESERVATION_BYTES: u64 = 24 * 1024 * 1024;

const TRANSACTION: &str = "transaction";
const DOWNLOAD: &str = "download";
const ARCHIVE: &str = "release.zip";
const RESTORED_RECORD: &str = "restored.json";
const RESTORED_SCHEMA: u32 = 1;
/// The update report of the last startup update, in the host's storage for this pack.
pub const REPORT_RECORD: &str = "report.json";
const MAX_RECORD_BYTES: usize = 64 * 1024;

/// The network boundary of a startup update: release metadata and archive downloads.
pub trait ReleaseDownloads: ReleaseSources {
    /// Stream `offer`'s archive into `output`, at most [`super::MAX_ARCHIVE_BYTES`]. The
    /// caller owns the file and verifies the bytes afterwards.
    ///
    /// # Errors
    /// Returns why the archive could not be obtained; `output` may then hold a prefix.
    fn download(
        &self,
        offer: &ReleaseOffer,
        output: &mut dyn io::Write,
    ) -> std::result::Result<(), SourceFailure>;
}

/// The step of a startup update that failed; spelled for the update report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupStep {
    /// Deciding what to install.
    Checking,
    /// Downloading and verifying the archive.
    Downloading,
    /// Replacing the tracked files.
    Installing,
}

impl StartupStep {
    /// Return the report's spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Checking => "checking",
            Self::Downloading => "downloading",
            Self::Installing => "installing",
        }
    }
}

/// What one startup update did. Every outcome leaves a pack ComfyUI can load.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartupOutcome {
    /// The newest version was installed; this start judges it.
    Updated {
        /// The version that was installed before.
        from: ReleaseVersion,
        /// The version now installed.
        to: ReleaseVersion,
        /// Where it came from.
        source: ReleaseSourceKind,
        /// Sources that did not answer.
        unchecked: Vec<ReleaseSourceKind>,
    },
    /// No newer version is published.
    UpToDate,
    /// The last update failed its first start, so the version before it is installed again.
    Restored(Restored),
    /// An update installed earlier is still being judged; nothing else is installed.
    Judging {
        /// The version being judged.
        version: String,
    },
    /// The folder is not one the host may replace, and is left exactly as it is.
    NotManaged(NotOwnedReason),
    /// No source answered, or the one that did offered nothing newer.
    CouldNotCheck {
        /// Sources that did not answer.
        unchecked: Vec<ReleaseSourceKind>,
    },
    /// SugarCubes' own updater is changing the folder right now.
    Busy,
    /// A step failed; the installed version is kept (restored first when it was touched).
    Failed {
        /// The step that failed.
        step: StartupStep,
        /// The shared refusal code, or `io` and `source` for filesystem and network failures.
        code: &'static str,
    },
    /// The worker was cancelled or ran out of time before anything was changed.
    Stopped,
    /// A newer version exists for a ComfyUI this host does not start; nothing was changed.
    Available {
        /// The newest version.
        version: ReleaseVersion,
        /// Sources that did not answer.
        unchecked: Vec<ReleaseSourceKind>,
    },
}

/// One pack folder ComfyUI is about to load and the host storage that updates it.
pub struct StartupUpdate {
    folder: PackFolder,
    transaction: PackTransaction,
    storage: PackTree,
}

impl StartupUpdate {
    /// Address `folder`, the pack folder as ComfyUI reaches it, and `storage`, a folder in the
    /// host's own storage that only this owner uses for this pack.
    ///
    /// # Errors
    /// Refuses relative or non-local paths.
    pub fn at(folder: &Path, storage: &Path) -> io::Result<Self> {
        Ok(Self {
            folder: PackFolder::at(folder)?,
            transaction: PackTransaction::at(&storage.join(TRANSACTION))?,
            storage: PackTree::at(storage)?,
        })
    }

    /// Settle the previous start's judgement, then install the newest version if one exists.
    ///
    /// `proceed` returns false once the worker is cancelled or past its deadline; the run then
    /// stops before its next step, never in the middle of replacing files.
    pub fn run(
        &self,
        sources: &dyn ReleaseDownloads,
        proceed: &dyn Fn() -> bool,
    ) -> StartupOutcome {
        self.record(self.run_locked(sources, proceed))
    }

    /// Log `outcome` and keep it as the update report in this pack's host storage, replacing
    /// the previous report; returns it. A report that cannot be written is logged, never fatal.
    pub fn record(&self, outcome: StartupOutcome) -> StartupOutcome {
        tracing::info!(
            target: "sugarsubstitute::pack_install",
            operation = "startup_update",
            outcome = ?outcome,
            "The SugarCubes startup update finished"
        );
        let written = outcome
            .report()
            .render()
            .map_err(io::Error::other)
            .and_then(|bytes| self.replace_record(REPORT_RECORD, &bytes));
        if let Err(cause) = written {
            tracing::warn!(
                target: "sugarsubstitute::pack_install",
                operation = "startup_update_report",
                error = %cause,
                "The SugarCubes update report could not be written"
            );
        }
        outcome
    }

    fn run_locked(
        &self,
        sources: &dyn ReleaseDownloads,
        proceed: &dyn Fn() -> bool,
    ) -> StartupOutcome {
        let _lock = match self.lock() {
            Ok(Some(lock)) => lock,
            Ok(None) => return StartupOutcome::Busy,
            Err(Refusal::NotManaged(reason)) => return StartupOutcome::NotManaged(reason),
            Err(Refusal::Failed(code)) => {
                return StartupOutcome::Failed { step: StartupStep::Checking, code };
            }
        };
        match self.transaction.settle(&self.folder, TrialEnding::Unwitnessed) {
            Ok(Some(restored)) => {
                return match self.remember_restored(&restored.version) {
                    Ok(()) => StartupOutcome::Restored(restored),
                    Err(cause) => failed(StartupStep::Installing, &cause),
                };
            }
            Ok(None) => {}
            Err(cause) => return failed(StartupStep::Installing, &cause),
        }
        match self.transaction.pending() {
            Ok(Some((_, _, version))) => return StartupOutcome::Judging { version },
            Ok(None) => {}
            Err(cause) => return failed(StartupStep::Installing, &cause),
        }
        if !proceed() {
            return StartupOutcome::Stopped;
        }
        let config = match self.release_sources() {
            Ok(config) => config,
            Err(outcome) => return outcome,
        };
        let decision = match self.folder.plan_update(&config, sources) {
            Ok(decision) => decision,
            Err(cause) => return failed(StartupStep::Checking, &cause),
        };
        let (offer, from, installed, unchecked) = match decision {
            UpdateDecision::Install { offer, from, installed, unchecked } => {
                (offer, from, installed, unchecked)
            }
            UpdateDecision::UpToDate { .. } => return StartupOutcome::UpToDate,
            UpdateDecision::NotManaged(reason) => return StartupOutcome::NotManaged(reason),
            UpdateDecision::CheckFailed { unchecked } => {
                return StartupOutcome::CouldNotCheck { unchecked };
            }
        };
        match self.restored_version() {
            Ok(Some(restored)) if restored == offer.version.to_string() => {
                return StartupOutcome::UpToDate;
            }
            Ok(_) => {}
            Err(cause) => return failed(StartupStep::Checking, &cause),
        }
        if !proceed() {
            return StartupOutcome::Stopped;
        }
        let archive = match self.download(sources, &offer) {
            Ok(archive) => archive,
            Err(code) => return StartupOutcome::Failed { step: StartupStep::Downloading, code },
        };
        if !proceed() {
            return StartupOutcome::Stopped;
        }
        let installed_now =
            self.transaction.install(&self.folder, archive, &config.pack_id, &offer, &installed);
        let discarded = self.discard_download();
        if let Err(cause) = installed_now {
            return failed(StartupStep::Installing, &cause);
        }
        if let Err(cause) = discarded {
            tracing::warn!(
                target: "sugarsubstitute::pack_install",
                operation = "discard_download",
                error = %cause,
                "The downloaded SugarCubes archive could not be removed"
            );
        }
        StartupOutcome::Updated { from, to: offer.version, source: offer.source, unchecked }
    }

    /// Check for a newer version of a pack in a ComfyUI this host does not start, changing
    /// nothing in the folder: its ComfyUI may already have the pack loaded, and SugarCubes'
    /// own updater or the next host-started backend installs it. Records the update report.
    pub fn check(&self, sources: &dyn ReleaseSources) -> StartupOutcome {
        let outcome = match (self.folder.ownership(), self.release_sources()) {
            (Err(cause), _) => failed(StartupStep::Checking, &cause),
            (Ok(Ownership::NotOwned(reason)), _) => StartupOutcome::NotManaged(reason),
            (Ok(Ownership::Owned(_)), Err(outcome)) => outcome,
            (Ok(Ownership::Owned(identity)), Ok(config)) if identity.is_pack(&config.pack_id) => {
                match newest_offer(&config, sources, identity.version) {
                    (Some(offer), unchecked) => {
                        StartupOutcome::Available { version: offer.version, unchecked }
                    }
                    (None, unchecked) if unchecked.is_empty() => StartupOutcome::UpToDate,
                    (None, unchecked) => StartupOutcome::CouldNotCheck { unchecked },
                }
            }
            (Ok(Ownership::Owned(_)), Ok(_)) => {
                StartupOutcome::NotManaged(NotOwnedReason::NotRegistryInstall)
            }
        };
        self.record(outcome)
    }

    /// Record one observation of the version the backend is running.
    ///
    /// # Errors
    /// Returns a journal or filesystem failure.
    pub fn observe(&self, event: TrialEvent) -> Result<()> {
        let Some((_, _, version)) = self.transaction.pending()? else {
            return Ok(());
        };
        self.transaction.observe(&version, event)
    }

    /// The page of the version being judged reported SugarCubes failing in this run, and its
    /// backend has since stopped: restore the previous version before another attempt starts
    /// it. Does nothing unless the judgement recorded that page failure. Records the report.
    ///
    /// # Errors
    /// Returns a journal or filesystem failure; the judgement stays for the next start.
    pub fn restore_failed_page(&self) -> Result<Option<Restored>> {
        if !matches!(self.transaction.pending()?, Some((JournalPhase::PageFailed, _, _))) {
            return Ok(None);
        }
        self.backend_stopped()
    }

    /// The backend ended before it was ready while an update was being judged: restore the
    /// previous version now, so the next attempt starts with it. Records the report.
    ///
    /// # Errors
    /// Returns a journal or filesystem failure; the judgement stays for the next start.
    pub fn backend_stopped(&self) -> Result<Option<Restored>> {
        let restored = self.transaction.settle(&self.folder, TrialEnding::BackendStopped)?;
        if let Some(restored) = &restored {
            self.remember_restored(&restored.version)?;
            self.record(StartupOutcome::Restored(restored.clone()));
        }
        Ok(restored)
    }

    fn lock(&self) -> std::result::Result<Option<PackLock>, Refusal> {
        match self.folder.ownership() {
            Ok(Ownership::Owned(_)) => {}
            Ok(Ownership::NotOwned(reason)) => return Err(Refusal::NotManaged(reason)),
            Err(cause) => return Err(Refusal::Failed(code(&cause))),
        }
        self.folder.tree.try_lock(UPDATE_LOCK_FILE).map_err(|_| Refusal::Failed("io"))
    }

    /// Read the installed pack's own description of its release sources.
    ///
    /// # Errors
    /// Returns the outcome to report when the description is missing or invalid.
    pub fn release_sources(&self) -> std::result::Result<ReleaseSourceConfig, StartupOutcome> {
        self.folder
            .tree
            .read(RELEASE_SOURCE_FILE, MAX_RELEASE_SOURCE_BYTES)
            .ok()
            .and_then(|bytes| ReleaseSourceConfig::parse(&bytes).ok())
            .ok_or(StartupOutcome::Failed { step: StartupStep::Checking, code: "release-source" })
    }

    fn download(
        &self,
        sources: &dyn ReleaseDownloads,
        offer: &ReleaseOffer,
    ) -> std::result::Result<impl Read + Seek + use<>, &'static str> {
        let directory = self.storage.child(DOWNLOAD).map_err(|_| "io")?;
        directory.create().map_err(|_| "io")?;
        let mut refused = None;
        let written = directory.replace_with(ARCHIVE, |output| {
            sources.download(offer, output).map_err(|failure| {
                refused = Some(failure);
                io::Error::other("The archive download failed")
            })
        });
        if refused.is_some() {
            return Err("source");
        }
        written.map_err(|_| "io")?;
        let length = directory.length(ARCHIVE).map_err(|_| "io")?;
        if length == 0 || length > super::MAX_ARCHIVE_BYTES {
            return Err("too-large");
        }
        if let Some(expected) = &offer.sha256 {
            let actual = directory.sha256(ARCHIVE).map_err(|_| "io")?;
            if !is_sha256(expected) || &actual != expected {
                return Err("digest-mismatch");
            }
        }
        directory.open(ARCHIVE).map_err(|_| "io")
    }

    fn discard_download(&self) -> io::Result<()> {
        self.storage.child(DOWNLOAD)?.remove_all()
    }

    fn remember_restored(&self, version: &str) -> Result<()> {
        let record = RestoredRecord { schema: RESTORED_SCHEMA, version: version.to_owned() };
        let bytes = serde_json::to_vec(&record).map_err(io::Error::other)?;
        Ok(self.replace_record(RESTORED_RECORD, &bytes)?)
    }

    /// Durably replace one small record in this pack's host storage.
    fn replace_record(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
        self.storage.create()?;
        self.storage.write(name, bytes)
    }

    fn restored_version(&self) -> Result<Option<String>> {
        if self.storage.kind(RESTORED_RECORD)? == NodeKind::Missing {
            return Ok(None);
        }
        let bytes = self.storage.read(RESTORED_RECORD, MAX_RECORD_BYTES)?;
        // An unreadable record only means a failed version may be tried once more.
        Ok(serde_json::from_slice::<RestoredRecord>(&bytes)
            .ok()
            .filter(|record| record.schema == RESTORED_SCHEMA)
            .map(|record| record.version))
    }
}

/// The version the host restored last, so it is not installed again.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoredRecord {
    schema: u32,
    version: String,
}

enum Refusal {
    NotManaged(NotOwnedReason),
    Failed(&'static str),
}

fn code(error: &PackInstallError) -> &'static str {
    error.code().map_or("io", ErrorCode::as_str)
}

fn failed(step: StartupStep, cause: &PackInstallError) -> StartupOutcome {
    tracing::warn!(
        target: "sugarsubstitute::pack_install",
        operation = "startup_update",
        step = step.as_str(),
        error = %cause,
        "A SugarCubes startup update step failed; the installed version is kept"
    );
    StartupOutcome::Failed { step, code: code(cause) }
}

impl StartupOutcome {
    /// The update report of this outcome, which also renders its one plain line and report
    /// lines through the localization owner.
    pub fn report(&self) -> PackUpdateReport {
        let kinds = |kinds: &[ReleaseSourceKind]| {
            kinds.iter().map(|kind| kind.as_str().to_owned()).collect()
        };
        match self {
            Self::Updated { from, to, source, unchecked } => PackUpdateReport {
                from: Some(from.to_string()),
                to: Some(to.to_string()),
                source: Some(source.as_str().to_owned()),
                unchecked: kinds(unchecked),
                ..PackUpdateReport::new(PackOutcome::Updated)
            },
            Self::UpToDate => PackUpdateReport::new(PackOutcome::UpToDate),
            Self::Restored(restored) => PackUpdateReport {
                from: Some(restored.version.clone()),
                to: Some(restored.restored_version.clone()),
                reason: Some(restored.reason.as_str().to_owned()),
                ..PackUpdateReport::new(PackOutcome::Restored)
            },
            Self::Judging { version } => PackUpdateReport {
                to: Some(version.clone()),
                ..PackUpdateReport::new(PackOutcome::Judging)
            },
            Self::NotManaged(reason) => PackUpdateReport {
                reason: Some(reason.as_str().to_owned()),
                ..PackUpdateReport::new(PackOutcome::NotManaged)
            },
            Self::CouldNotCheck { unchecked } => PackUpdateReport {
                unchecked: kinds(unchecked),
                ..PackUpdateReport::new(PackOutcome::CouldNotCheck)
            },
            Self::Busy => PackUpdateReport::new(PackOutcome::Busy),
            Self::Failed { step, code } => PackUpdateReport {
                step: Some(step.as_str().to_owned()),
                code: Some((*code).to_owned()),
                ..PackUpdateReport::new(PackOutcome::Failed)
            },
            Self::Stopped => PackUpdateReport::new(PackOutcome::Stopped),
            Self::Available { version, unchecked } => PackUpdateReport {
                to: Some(version.to_string()),
                unchecked: kinds(unchecked),
                ..PackUpdateReport::new(PackOutcome::Available)
            },
        }
    }
}
