// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide what the main window's caption says about updates, the close save and one-time
//! messages, from what the application reports. This owner holds no native resources; the
//! caption peers show its [`UpdateCaptionView`] and route their clicks back through
//! [`CaptionUpdates::activate`].
use crate::WorkflowActivity;
use std::borrow::Cow;
use sugarsubstitute_application::workspace_session::close_save::{
    CloseSaveProgress, NotSavedLastTime, UnsavedItem,
};
use sugarsubstitute_contracts::{
    localization::{Messages, updates::UpdateMessages},
    update::status::UpdateStatus,
};

/// Caption peer showing the update button.
pub(crate) const UPDATE: u16 = 5;
/// Caption peer offering the restart that applies a ready update.
pub(crate) const OFFER: u16 = 6;
/// Caption peer showing the saving state or a one-time message.
pub(crate) const LINE: u16 = 7;
/// Caption peer linking a one-time message to more information.
pub(crate) const LINK: u16 = 8;
/// Caption peer dismissing the one-time message.
pub(crate) const DISMISS: u16 = 9;

/// Whether caption peer `id` belongs to this owner.
pub(crate) const fn owns(id: u16) -> bool {
    UPDATE <= id && id <= DISMISS
}

/// The update button's state, which also selects its tint.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum UpdateGlyph {
    /// No update is ready and nothing failed; the button is hidden.
    #[default]
    Hidden,
    /// An update is ready; clicking offers the restart that applies it.
    Ready,
    /// The latest attempt failed; clicking opens its report.
    Warning,
}

/// The application's facts for one turn, reduced to what the caption shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct UpdateFeed {
    pub(crate) glyph: UpdateGlyph,
    pub(crate) saving: bool,
    pub(crate) workflow: WorkflowActivity,
}

impl UpdateFeed {
    /// Reduce the host's update status, close-save progress and workflow activity.
    /// A ready update outranks a later failed check, because restarting still applies it.
    pub(crate) fn read(
        status: Option<&UpdateStatus>,
        closing: Option<CloseSaveProgress>,
        workflow: WorkflowActivity,
    ) -> Self {
        let glyph = match status {
            Some(status) if status.ready().is_some() => UpdateGlyph::Ready,
            Some(status) if status.failure().is_some() => UpdateGlyph::Warning,
            _ => UpdateGlyph::Hidden,
        };
        let saving = matches!(
            closing,
            Some(CloseSaveProgress::Saving { .. } | CloseSaveProgress::Recording(_))
        );
        Self { glyph, saving, workflow }
    }
}

/// A one-time message shown as one line the user can dismiss.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Notice {
    /// The previous close did not keep these.
    NotSaved {
        /// Workflow changes were lost.
        workspace: bool,
        /// Input canvas edits were lost.
        inputs: bool,
    },
    /// The first start on a new version, with where its release notes are.
    Updated {
        /// The version now running.
        version: String,
        /// The release notes, opened by the caption's link when present.
        changelog: Option<url::Url>,
    },
    /// A new version did not start, so the one before it is running.
    PreviousVersionRunning,
    /// The previous version is running while the newer one is repaired.
    Repairing,
}

impl Notice {
    /// The line for what the previous close lost.
    pub(crate) fn not_saved(notice: &NotSavedLastTime) -> Self {
        let lost = |wanted: fn(&UnsavedItem) -> bool| notice.items.iter().any(wanted);
        Self::NotSaved {
            workspace: lost(|item| matches!(item, UnsavedItem::Workspace(_))),
            inputs: lost(|item| matches!(item, UnsavedItem::InputCanvas(_))),
        }
    }

    /// The line for a start-up update message.
    pub(crate) fn update(notice: crate::UpdateNotice) -> Self {
        match notice {
            crate::UpdateNotice::Updated { version, changelog } => {
                Self::Updated { version, changelog }
            }
            crate::UpdateNotice::PreviousVersionRunning => Self::PreviousVersionRunning,
            crate::UpdateNotice::Repairing => Self::Repairing,
        }
    }
}

/// What a click asks of the application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UpdateIntent {
    /// Open the report of the failed update attempt.
    OpenReport,
    /// Run the ordinary close and relaunch so the ready update applies.
    RestartToApply,
    /// Open the running version's release notes in the user's browser.
    OpenChangelog(url::Url),
}

/// One caption peer's current state. `label` is the peer's stable localized name; `text` is
/// what it shows, which differs only for a message that names a version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PeerView {
    pub(crate) visible: bool,
    pub(crate) label: &'static str,
    pub(crate) text: Cow<'static, str>,
}

impl PeerView {
    fn shown(visible: bool, label: &'static str) -> Self {
        Self { visible, label, text: Cow::Borrowed(label) }
    }
}

/// Everything the caption shows for this owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UpdateCaptionView {
    pub(crate) glyph: UpdateGlyph,
    pub(crate) update: PeerView,
    pub(crate) offer: PeerView,
    pub(crate) line: PeerView,
    pub(crate) link: PeerView,
    pub(crate) dismiss: PeerView,
}

impl UpdateCaptionView {
    /// The peer with caption id `id`.
    pub(crate) fn peer(&self, id: u16) -> Option<&PeerView> {
        match id {
            UPDATE => Some(&self.update),
            OFFER => Some(&self.offer),
            LINE => Some(&self.line),
            LINK => Some(&self.link),
            DISMISS => Some(&self.dismiss),
            _ => None,
        }
    }
}

/// The caption's update state for one main window.
#[derive(Debug, Default)]
pub(crate) struct CaptionUpdates {
    feed: UpdateFeed,
    offer_open: bool,
    /// One-time messages, shown one at a time in arrival order.
    notices: std::collections::VecDeque<Notice>,
}

impl CaptionUpdates {
    /// Adopt this turn's facts; true when the caption changes.
    pub(crate) fn observe(&mut self, feed: UpdateFeed) -> bool {
        let offer_open = self.offer_open && feed.glyph == UpdateGlyph::Ready && !feed.saving;
        let changed = feed != self.feed || offer_open != self.offer_open;
        self.feed = feed;
        self.offer_open = offer_open;
        changed
    }

    /// Show `notice` until the user dismisses it, after any message already waiting. A repeat
    /// of a waiting message is ignored. Returns whether the caption changes.
    pub(crate) fn notify(&mut self, notice: Notice) -> bool {
        if self.notices.contains(&notice) {
            return false;
        }
        self.notices.push_back(notice);
        self.notices.len() == 1
    }

    /// Handle a click on caption peer `id`, acting only on what the caption shows now, so a
    /// click queued before a change does nothing. Returns whether the caption changed and what
    /// the application must do.
    pub(crate) fn activate(&mut self, id: u16) -> (bool, Option<UpdateIntent>) {
        if !self.shows(id) {
            return (false, None);
        }
        match (id, self.feed.glyph) {
            (UPDATE, UpdateGlyph::Ready) => {
                self.offer_open = !self.offer_open;
                (true, None)
            }
            (UPDATE, UpdateGlyph::Warning) => (false, Some(UpdateIntent::OpenReport)),
            (OFFER, _) => {
                self.offer_open = false;
                (true, Some(UpdateIntent::RestartToApply))
            }
            (LINK, _) => match self.notices.front() {
                Some(Notice::Updated { changelog: Some(url), .. }) => {
                    (false, Some(UpdateIntent::OpenChangelog(url.clone())))
                }
                _ => (false, None),
            },
            (DISMISS, _) => {
                self.notices.pop_front();
                (true, None)
            }
            _ => (false, None),
        }
    }

    /// Whether caption peer `id` is shown now.
    fn shows(&self, id: u16) -> bool {
        let quiet = !self.feed.saving;
        match id {
            UPDATE => quiet && self.feed.glyph != UpdateGlyph::Hidden,
            OFFER => quiet && self.feed.glyph == UpdateGlyph::Ready && self.offer_open,
            LINE => !quiet || !self.notices.is_empty(),
            LINK => {
                quiet
                    && matches!(
                        self.notices.front(),
                        Some(Notice::Updated { changelog: Some(_), .. })
                    )
            }
            DISMISS => quiet && !self.notices.is_empty(),
            _ => false,
        }
    }

    /// The caption in `words`' language. While the close save runs, the line says so and
    /// nothing else is offered.
    pub(crate) fn view(&self, words: &'static Messages) -> UpdateCaptionView {
        let updates = words.updates;
        let saving = self.feed.saving;
        let glyph = if saving { UpdateGlyph::Hidden } else { self.feed.glyph };
        let restart = if self.feed.workflow == WorkflowActivity::Running {
            updates.restart_after_workflow
        } else {
            updates.restart
        };
        let line = match self.notices.front().filter(|_| !saving) {
            None => PeerView::shown(self.shows(LINE), words.closing.saving),
            Some(Notice::NotSaved { workspace, inputs }) => PeerView::shown(
                true,
                words
                    .closing
                    .not_saved(*workspace, *inputs)
                    .unwrap_or(words.closing.not_saved_both),
            ),
            Some(Notice::Updated { version, .. }) => PeerView {
                visible: true,
                label: updates.updated,
                text: Cow::Owned(UpdateMessages::with_version(updates.updated, version)),
            },
            Some(Notice::PreviousVersionRunning) => PeerView::shown(true, updates.previous_running),
            Some(Notice::Repairing) => PeerView::shown(true, updates.repairing),
        };
        UpdateCaptionView {
            glyph,
            update: PeerView::shown(
                self.shows(UPDATE),
                if glyph == UpdateGlyph::Warning { updates.failed } else { updates.ready },
            ),
            offer: PeerView::shown(self.shows(OFFER), restart),
            line,
            link: PeerView::shown(self.shows(LINK), updates.whats_new),
            dismiss: PeerView::shown(self.shows(DISMISS), updates.dismiss),
        }
    }
}

#[cfg(test)]
#[path = "main_updates_tests.rs"]
mod tests;
