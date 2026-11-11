// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Take part in the application's close save while the canvas stays open.
use super::*;
use sugarsubstitute_application::workspace_session::close_save::SaveVerdict;

impl EmbeddedCanvas {
    /// Begin the close save's final input-document save, bounded by the graceful `cutoff` on
    /// the execution clock, while the canvas stays open. Routed gestures are refused from here
    /// on, so the saved documents are the ones the window last showed.
    pub(in crate::startup) fn begin_close_save(&mut self, cutoff: Moment) {
        self.close_saving = true;
        if let Some(session) = &mut self.session {
            session.begin_final_input_save(cutoff);
        }
    }

    /// What the final input-document save kept at `now`; `None` while it runs. A canvas that
    /// never started holds no authored documents.
    pub(in crate::startup) fn input_close_verdict(&self, now: Moment) -> Option<SaveVerdict> {
        match &self.session {
            None => Some(SaveVerdict::Saved),
            Some(session) => session.input_close_verdict(now),
        }
    }
}
