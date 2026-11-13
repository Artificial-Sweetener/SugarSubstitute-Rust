// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the Main caption's update peers showing the caption owner's current view.
use super::*;
use crate::main_updates::{CaptionUpdates, Notice, UpdateCaptionView, UpdateFeed, UpdateIntent};
use sugarsubstitute_contracts::localization::{Messages, messages};

/// The caption owner and the language it speaks.
pub(super) struct CaptionText {
    owner: CaptionUpdates,
    words: &'static Messages,
}

impl Default for CaptionText {
    fn default() -> Self {
        Self { owner: CaptionUpdates::default(), words: messages("en") }
    }
}

impl CaptionText {
    /// The caption owner's view in the current language.
    pub(super) fn view(&self) -> UpdateCaptionView {
        self.owner.view(self.words)
    }
}

impl MainActor {
    /// Adopt this turn's update, close-save and workflow facts in `words`' language, and the
    /// previous close's losses once; repaint the caption when anything changed.
    pub(crate) fn observe_updates(
        &mut self,
        feed: UpdateFeed,
        words: &'static Messages,
        notices: Vec<Notice>,
    ) -> Result<(), DesktopError> {
        let language = !std::ptr::eq(words, self.updates.words);
        self.updates.words = words;
        let mut changed = self.updates.owner.observe(feed) || language;
        for notice in notices {
            changed |= self.updates.owner.notify(notice);
        }
        if changed { self.present_caption_updates() } else { Ok(()) }
    }

    /// Whether this window is closing or has retired.
    pub(crate) fn closing(&self) -> bool {
        self.admission.closing() || self.retired
    }

    /// Keep this window from being destroyed while a window it owns is open, because Windows
    /// destroys owned windows with their owner, behind their own owner's back.
    pub(crate) fn hold_for_owned_window(&mut self, open: bool) {
        self.owned_window_open = open;
    }

    /// Whether the window has been shown, so a one-time message reaches the user.
    pub(crate) fn shows_messages(&self) -> bool {
        self.revealed && self.controls.is_some() && !self.admission.closing() && !self.retired
    }

    /// Handle a click on update caption peer `id`; returns what the application must do.
    pub(crate) fn activate_update(
        &mut self,
        id: u16,
    ) -> Result<Option<UpdateIntent>, DesktopError> {
        if self.admission.closing() || self.retired {
            return Ok(None);
        }
        let (changed, intent) = self.updates.owner.activate(id);
        if changed {
            self.present_caption_updates()?;
        }
        Ok(intent)
    }

    /// Lay the caption out again with the current view.
    fn present_caption_updates(&mut self) -> Result<(), DesktopError> {
        if self.controls.is_none() || self.admission.closing() || self.retired {
            return Ok(());
        }
        self.resize_caption()?;
        match &self.controls {
            Some(owner) => owner.raise_main(),
            None => Ok(()),
        }
    }

    /// Show the current caption without moving the browser.
    pub(super) fn resize_caption(&mut self) -> Result<(), DesktopError> {
        let controls = self.caption_controls()?;
        let view = self.updates.view();
        if let Some(owner) = &mut self.controls {
            owner.present_caption(&controls, &view)?;
        }
        Ok(())
    }
}
