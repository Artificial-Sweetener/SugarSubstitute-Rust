// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide what a close kept: the workspace and the input canvas, each saved, stale or failed.
//!
//! Every way the application ends with the user's work in place (the window's close, restart
//! to apply an update, the operating system's session end) runs one close save. It begins with
//! the shutdown budget's graceful cutoff, collects one [`SaveVerdict`] from each owner of
//! authored state, and settles into a [`CloseSaveOutcome`]. An owner that has not answered by
//! the cutoff is [`SaveVerdict::Stale`]; passing the cutoff never counts as saved. The exit
//! intent is admitted only once the outcome exists and, when anything was not saved, its
//! record is durable, so the next start can say what was lost.
use sugarsubstitute_contracts::execution::Moment;

/// Why authored state could not be written at close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveFailure {
    /// The store refused or failed the write; the previous copy is kept.
    Storage,
    /// The state could not be captured from its live owner.
    Capture,
    /// Unsupported originals on disk are protected, so nothing may replace them.
    Protected,
}

impl SaveFailure {
    /// Stable spelling used by the persisted close-save record.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Storage => "storage",
            Self::Capture => "capture",
            Self::Protected => "protected",
        }
    }

    /// Parse the persisted spelling.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "storage" => Some(Self::Storage),
            "capture" => Some(Self::Capture),
            "protected" => Some(Self::Protected),
            _ => None,
        }
    }
}

/// What one owner of authored state kept at close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveVerdict {
    /// Everything the user authored is durable.
    Saved,
    /// Changes made since `since`, on the execution clock, were not written before the cutoff.
    Stale {
        /// When the oldest unsaved change was made.
        since: Moment,
    },
    /// The write failed; the previous durable copy is kept.
    Failed(SaveFailure),
}

impl SaveVerdict {
    /// Whether this owner kept everything.
    pub const fn saved(self) -> bool {
        matches!(self, Self::Saved)
    }
}

/// Why the application is closing; it selects nothing about the save, only what follows it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseReason {
    /// The user closed the window.
    Close,
    /// The user chose to restart so a prepared update applies; relaunch follows the outcome.
    RestartToApply,
    /// The operating system is ending the session.
    SessionEnding {
        /// Whether the operating system will end the process without waiting.
        critical: bool,
    },
}

impl CloseReason {
    /// Stable spelling used by the persisted close-save record.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Close => "close",
            Self::RestartToApply => "restart-to-apply",
            Self::SessionEnding { critical: false } => "session-ending",
            Self::SessionEnding { critical: true } => "session-ending-critical",
        }
    }

    /// Parse the persisted spelling.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "close" => Some(Self::Close),
            "restart-to-apply" => Some(Self::RestartToApply),
            "session-ending" => Some(Self::SessionEnding { critical: false }),
            "session-ending-critical" => Some(Self::SessionEnding { critical: true }),
            _ => None,
        }
    }
}

/// What one close kept, per owner of authored state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseSaveOutcome {
    /// Why the application closed.
    pub reason: CloseReason,
    /// Workflow tabs, widget values, views and window placement.
    pub workspace: SaveVerdict,
    /// Input canvas documents: layers, masks, selection and their synced state.
    pub inputs: SaveVerdict,
}

impl CloseSaveOutcome {
    /// Whether everything authored is durable, so no record is written.
    pub const fn saved(&self) -> bool {
        self.workspace.saved() && self.inputs.saved()
    }
}

/// What the window shows while it stays up for the close save.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseSaveProgress {
    /// Saves are running; the window shows "Saving your workspace…".
    Saving {
        /// Why the application is closing.
        reason: CloseReason,
        /// When the close save began, on the execution clock.
        started: Moment,
        /// The graceful cutoff after which unsaved work counts as stale.
        cutoff: Moment,
    },
    /// Both owners answered; the record of anything unsaved is being made durable.
    Recording(CloseSaveOutcome),
    /// The outcome is final and the application exits.
    Settled(CloseSaveOutcome),
}

/// The single close-save transaction: started once, tightened never renewed, settled once.
#[derive(Debug)]
pub struct CloseSave {
    reason: CloseReason,
    started: Moment,
    cutoff: Moment,
    workspace: Option<SaveVerdict>,
    inputs: Option<SaveVerdict>,
    recorded: bool,
}

impl CloseSave {
    /// Begin the close save at `now`, bounded by the shutdown budget's graceful `cutoff`.
    pub const fn begin(reason: CloseReason, now: Moment, cutoff: Moment) -> Self {
        Self { reason, started: now, cutoff, workspace: None, inputs: None, recorded: false }
    }

    /// Why the application is closing.
    pub const fn reason(&self) -> CloseReason {
        self.reason
    }

    /// The graceful cutoff, which a later budget may tighten but never extend.
    pub const fn cutoff(&self) -> Moment {
        self.cutoff
    }

    /// Adopt an earlier cutoff; a later one is ignored.
    pub fn tighten(&mut self, cutoff: Moment) {
        if cutoff < self.cutoff {
            self.cutoff = cutoff;
        }
    }

    /// Record the workspace owner's verdict; the first answer stands.
    pub fn workspace(&mut self, verdict: SaveVerdict) {
        self.workspace.get_or_insert(verdict);
    }

    /// Record the input canvas owner's verdict; the first answer stands.
    pub fn inputs(&mut self, verdict: SaveVerdict) {
        self.inputs.get_or_insert(verdict);
    }

    /// Whether the input canvas owner has answered.
    pub const fn inputs_answered(&self) -> bool {
        self.inputs.is_some()
    }

    /// Whether the workspace owner has answered.
    pub const fn workspace_answered(&self) -> bool {
        self.workspace.is_some()
    }

    /// Settle every unanswered owner as stale once `now` reaches the cutoff. Unanswered work
    /// is stale since the close began, the latest moment its owner could have saved it.
    pub fn expire(&mut self, now: Moment) {
        if now < self.cutoff {
            return;
        }
        let stale = SaveVerdict::Stale { since: self.started };
        self.workspace.get_or_insert(stale);
        self.inputs.get_or_insert(stale);
    }

    /// The outcome, once both owners answered.
    pub fn outcome(&self) -> Option<CloseSaveOutcome> {
        Some(CloseSaveOutcome {
            reason: self.reason,
            workspace: self.workspace?,
            inputs: self.inputs?,
        })
    }

    /// Note that the record of this outcome is durable, or that none was needed.
    pub fn recorded(&mut self) {
        self.recorded = self.outcome().is_some();
    }

    /// Whether the exit intent may be admitted: the outcome exists and is recorded.
    pub fn settled(&self) -> bool {
        self.recorded
    }

    /// When the host must next look, if any owner has yet to answer.
    pub fn deadline(&self) -> Option<Moment> {
        self.outcome().is_none().then_some(self.cutoff)
    }

    /// The presentation state for the window while it stays up.
    pub fn progress(&self) -> CloseSaveProgress {
        match self.outcome() {
            None => CloseSaveProgress::Saving {
                reason: self.reason,
                started: self.started,
                cutoff: self.cutoff,
            },
            Some(outcome) if self.recorded => CloseSaveProgress::Settled(outcome),
            Some(outcome) => CloseSaveProgress::Recording(outcome),
        }
    }
}

/// One owner's loss at the previous close, as the next start reports it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsavedItem {
    /// Workflow tabs and their values were not saved.
    Workspace(UnsavedCause),
    /// Input canvas documents were not saved.
    InputCanvas(UnsavedCause),
}

/// Why an item was not saved at the previous close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsavedCause {
    /// The application had to close before the latest changes were written.
    Interrupted {
        /// How long the oldest unsaved change had waited when the close cut it off.
        unsaved_for: std::time::Duration,
    },
    /// Writing failed.
    Failed(SaveFailure),
}

/// The notice the next start shows once: what the previous close did not keep.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotSavedLastTime {
    /// Why the previous run closed.
    pub reason: CloseReason,
    /// Every owner whose latest changes were lost, never empty.
    pub items: Vec<UnsavedItem>,
}

impl NotSavedLastTime {
    /// The notice for a recorded outcome whose close began at `started` and was cut off at
    /// `ended`, or `None` when it kept everything.
    pub fn from_outcome(outcome: &CloseSaveOutcome, ended: Moment) -> Option<Self> {
        let cause = |verdict: SaveVerdict| match verdict {
            SaveVerdict::Saved => None,
            SaveVerdict::Stale { since } => Some(UnsavedCause::Interrupted {
                unsaved_for: ended.checked_duration_since(since).unwrap_or_default(),
            }),
            SaveVerdict::Failed(failure) => Some(UnsavedCause::Failed(failure)),
        };
        let items: Vec<_> = [
            cause(outcome.workspace).map(UnsavedItem::Workspace),
            cause(outcome.inputs).map(UnsavedItem::InputCanvas),
        ]
        .into_iter()
        .flatten()
        .collect();
        (!items.is_empty()).then_some(Self { reason: outcome.reason, items })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(seconds: u64) -> Moment {
        Moment::from_nanos(seconds * 1_000_000_000)
    }

    #[test]
    fn an_owner_unanswered_at_the_cutoff_is_stale_never_saved() -> Result<(), &'static str> {
        let mut save = CloseSave::begin(CloseReason::Close, at(1), at(11));
        save.workspace(SaveVerdict::Saved);
        save.expire(at(10));
        assert_eq!(save.outcome(), None);
        assert_eq!(save.deadline(), Some(at(11)));
        save.expire(at(11));
        let outcome = save.outcome().ok_or("both owners settle at the cutoff")?;
        assert_eq!(outcome.workspace, SaveVerdict::Saved);
        assert_eq!(outcome.inputs, SaveVerdict::Stale { since: at(1) });
        assert!(!outcome.saved());
        assert_eq!(save.deadline(), None);
        Ok(())
    }

    #[test]
    fn the_exit_waits_for_the_outcome_and_its_record() {
        let mut save = CloseSave::begin(CloseReason::RestartToApply, at(0), at(10));
        assert!(matches!(save.progress(), CloseSaveProgress::Saving { .. }));
        save.recorded();
        assert!(!save.settled(), "no record exists before both answers");
        save.inputs(SaveVerdict::Failed(SaveFailure::Storage));
        save.workspace(SaveVerdict::Saved);
        assert!(matches!(save.progress(), CloseSaveProgress::Recording(_)));
        assert!(!save.settled());
        save.recorded();
        assert!(save.settled());
        assert!(matches!(save.progress(), CloseSaveProgress::Settled(outcome) if !outcome.saved()));
    }

    #[test]
    fn a_first_answer_stands_and_the_cutoff_only_tightens() {
        let mut save = CloseSave::begin(CloseReason::Close, at(0), at(10));
        save.tighten(at(12));
        assert_eq!(save.cutoff(), at(10));
        save.tighten(at(4));
        assert_eq!(save.cutoff(), at(4));
        save.inputs(SaveVerdict::Saved);
        save.inputs(SaveVerdict::Failed(SaveFailure::Capture));
        save.expire(at(4));
        assert_eq!(save.outcome().map(|outcome| outcome.inputs), Some(SaveVerdict::Saved));
    }

    #[test]
    fn the_notice_lists_only_what_was_lost() -> Result<(), &'static str> {
        let outcome = CloseSaveOutcome {
            reason: CloseReason::SessionEnding { critical: true },
            workspace: SaveVerdict::Stale { since: at(2) },
            inputs: SaveVerdict::Saved,
        };
        let notice = NotSavedLastTime::from_outcome(&outcome, at(7)).ok_or("a loss")?;
        assert_eq!(
            notice.items,
            vec![UnsavedItem::Workspace(UnsavedCause::Interrupted {
                unsaved_for: Duration::from_secs(5)
            })]
        );
        let saved = CloseSaveOutcome { workspace: SaveVerdict::Saved, ..outcome };
        assert_eq!(NotSavedLastTime::from_outcome(&saved, at(7)), None);
        Ok(())
    }

    #[test]
    fn persisted_spellings_round_trip() {
        for reason in [
            CloseReason::Close,
            CloseReason::RestartToApply,
            CloseReason::SessionEnding { critical: false },
            CloseReason::SessionEnding { critical: true },
        ] {
            assert_eq!(CloseReason::parse(reason.as_str()), Some(reason));
        }
        for failure in [SaveFailure::Storage, SaveFailure::Capture, SaveFailure::Protected] {
            assert_eq!(SaveFailure::parse(failure.as_str()), Some(failure));
        }
    }
}
