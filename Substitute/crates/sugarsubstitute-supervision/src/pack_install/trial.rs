// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Judge a just-installed pack by what its first start shows, with three verdicts.
//!
//! Only a failure of the new pack itself counts against it: a backend that ends before it is
//! ready with every SugarCubes node registered, or a first page that reports a load failure.
//! A start the host did not witness (power cut, forced kill, the user closing the app first)
//! is no verdict: the new version stays and is judged at the next start, except that three
//! unwitnessed endings in a row count as one failed start, so a pack that takes the whole
//! start down with it can never loop. A confirmed version drops its backup; a failed one is
//! restored from it.

use super::error::Result;
use super::replacement::{JournalPhase, TrackedReplacement};
use crate::durable_storage::pack_tree::PackTree;

/// Unwitnessed trial endings that together count as one failed start.
pub const UNWITNESSED_ENDINGS_LIMIT: u32 = 3;

/// One observation of a just-installed version, in the shared vectors' vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrialEvent {
    /// The backend finished a full start; whether every SugarCubes node registered.
    BackendReady {
        /// Whether ComfyUI registered every SugarCubes node class.
        nodes_registered: bool,
    },
    /// The backend started but its start could not be observed.
    BackendStartUnobservable,
    /// The first page loaded and established its session.
    PageLoaded {
        /// Whether the page found every SugarCubes node registered.
        nodes_registered: bool,
    },
    /// The first page reported a load failure or never established its session.
    PageFailed,
}

/// How the run that held a just-installed version ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrialEnding {
    /// The host saw the backend end, or start again, with this version's judgement open.
    BackendStopped,
    /// The run ended without the host witnessing the backend's fate.
    Unwitnessed,
}

/// Why a previous version was restored; spelled as in the shared vectors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreReason {
    /// The new version's backend never became ready.
    DidNotStart,
    /// The new version's first page failed.
    PageDidNotLoad,
    /// The replacement was interrupted.
    Interrupted,
}

impl RestoreReason {
    /// Return the shared spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DidNotStart => "did-not-start",
            Self::PageDidNotLoad => "page-did-not-load",
            Self::Interrupted => "interrupted",
        }
    }
}

/// A previous version put back by [`TrackedReplacement::settle`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Restored {
    /// The version that was removed.
    pub version: String,
    /// The version that is installed again.
    pub restored_version: String,
    /// Why.
    pub reason: RestoreReason,
}

impl TrackedReplacement {
    /// Record one observation of `running_version`; an observation of another version, or
    /// one that arrives after the judgement closed, changes nothing.
    pub(crate) fn observe(&self, running_version: &str, event: TrialEvent) -> Result<()> {
        let Some(pending) = self.pending()? else {
            return Ok(());
        };
        if pending.to_version != running_version {
            return Ok(());
        }
        match (pending.phase, event) {
            (JournalPhase::Applied, TrialEvent::BackendReady { nodes_registered: true }) => {
                self.advance(JournalPhase::Started)
            }
            (JournalPhase::Applied, TrialEvent::BackendStartUnobservable) => {
                self.advance(JournalPhase::Unobserved)
            }
            (JournalPhase::Started, TrialEvent::PageLoaded { .. })
            | (JournalPhase::Unobserved, TrialEvent::PageLoaded { nodes_registered: true }) => {
                tracing::info!(
                    target: "sugarsubstitute::pack_install",
                    operation = "confirm_update",
                    version = running_version,
                    "The SugarCubes update loaded its page and is kept"
                );
                self.confirm()
            }
            (JournalPhase::Unobserved, TrialEvent::PageLoaded { nodes_registered: false })
            | (JournalPhase::Started | JournalPhase::Unobserved, TrialEvent::PageFailed) => {
                self.advance(JournalPhase::PageFailed)
            }
            _ => Ok(()),
        }
    }

    /// Settle a judgement left open by the previous run, restoring `target` when the new
    /// version failed; returns what was restored.
    pub(crate) fn settle(
        &self,
        target: &PackTree,
        ending: TrialEnding,
    ) -> Result<Option<Restored>> {
        let Some(mut journal) = self.journal()? else {
            return Ok(None);
        };
        let reason = match (journal.phase, ending) {
            (JournalPhase::BackingUp, _) => {
                self.discard_unstarted()?;
                return Ok(None);
            }
            (JournalPhase::Started | JournalPhase::Unobserved, _) => return Ok(None),
            (JournalPhase::Replacing, _) => RestoreReason::Interrupted,
            (JournalPhase::PageFailed, _) => RestoreReason::PageDidNotLoad,
            (JournalPhase::Applied, TrialEnding::BackendStopped) => RestoreReason::DidNotStart,
            (JournalPhase::Applied, TrialEnding::Unwitnessed) => {
                journal.unwitnessed_endings += 1;
                if journal.unwitnessed_endings < UNWITNESSED_ENDINGS_LIMIT {
                    self.store(&journal)?;
                    return Ok(None);
                }
                RestoreReason::DidNotStart
            }
        };
        self.roll_back(target)?;
        tracing::warn!(
            target: "sugarsubstitute::pack_install",
            operation = "restore_previous",
            version = %journal.to_version,
            restored = %journal.from_version,
            reason = reason.as_str(),
            "The previous SugarCubes version was restored"
        );
        Ok(Some(Restored {
            version: journal.to_version,
            restored_version: journal.from_version,
            reason,
        }))
    }
}
