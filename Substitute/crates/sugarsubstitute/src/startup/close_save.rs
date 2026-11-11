// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the window up until everything authored is durable, or the record says what is not.
//!
//! [`StartupHost::request_close`] is the one entry point for every close that keeps the user's
//! work: the window's close button (through the desktop's close observation), restart to apply
//! an update and the operating system's session end. Each passes its [`CloseReason`]; all run
//! the same close save. Once the shutdown budget is measured, the close save begins the input
//! canvas's final save and the workspace's final capture together, both bounded by the
//! graceful cutoff, while the window stays up showing [`CloseSaveProgress`]. The exit intent is
//! admitted only after both answered and, when anything was not saved, its record is durable.
//! Restart to apply relaunches only after [`StartupHost::close_save_outcome`] exists.
use super::*;
use sugarsubstitute_application::workspace_session::close_save::{
    CloseReason, CloseSave, CloseSaveOutcome, CloseSaveProgress, NotSavedLastTime,
};
use sugarsubstitute_contracts::{execution::Moment, lifecycle::ShutdownBudget};

/// A test stand-in for the input canvas's final save: when it began and what it answers.
#[cfg(all(test, windows, feature = "native-composition"))]
#[derive(Default)]
pub(super) struct InputSaveProbe {
    pub(super) begun: Option<Moment>,
    pub(super) verdict:
        Option<sugarsubstitute_application::workspace_session::close_save::SaveVerdict>,
}

impl<B: StartupBackend> StartupHost<B> {
    /// Close the application for `reason`, keeping everything authored: the single entry point
    /// for the close button, restart to apply and session end. The first reason stands; a later
    /// request only advances the close already under way. The window stays up while
    /// [`Self::close_save_progress`] reports saving.
    /// # Errors
    /// Preserves the close transaction's first measurement or cleanup failure.
    pub fn request_close(&mut self, reason: CloseReason) -> Result<(), Box<dyn Error>> {
        if !self.closing {
            self.close_reason = reason;
        }
        self.close_owned()
    }

    /// What the window shows while it stays up for the close save; `None` before close began.
    pub fn close_save_progress(&self) -> Option<CloseSaveProgress> {
        self.close_save.as_ref().map(CloseSave::progress)
    }

    /// The close save's final outcome, once its record is durable or none was needed.
    pub fn close_save_outcome(&self) -> Option<CloseSaveOutcome> {
        self.close_save.as_ref().filter(|save| save.settled()).and_then(CloseSave::outcome)
    }

    /// Take, once, what the previous close did not keep, for the start-up notice surface.
    pub fn take_unsaved_notice(&mut self) -> Option<NotSavedLastTime> {
        #[cfg(all(windows, feature = "native-composition"))]
        return self.workspace.as_mut().and_then(workspace::WorkspaceWork::take_notice);
        #[cfg(not(all(windows, feature = "native-composition")))]
        None
    }

    /// When the close save must next look, while an owner has yet to answer.
    pub(super) fn close_save_deadline(&self) -> Option<Moment> {
        self.close_save.as_ref().and_then(CloseSave::deadline)
    }

    /// Begin the close save under `budget` on the first call, then collect each owner's
    /// verdict and record a loss. Returns whether the exit intent may be admitted. A record
    /// that cannot be submitted or written is logged and never holds the exit.
    /// # Errors
    /// Reserved for the close transaction; recording failures are logged, not returned.
    pub(super) fn advance_close_save(
        &mut self,
        budget: ShutdownBudget,
        now: Moment,
    ) -> Result<bool, Box<dyn Error>> {
        if self.close_save.is_none() {
            self.close_save = Some(CloseSave::begin(self.close_reason, now, budget.graceful_until));
            tracing::info!(
                operation = "close_save",
                reason = self.close_reason.as_str(),
                "Saving the workspace before closing"
            );
            #[cfg(all(windows, feature = "native-composition"))]
            if let Some(canvas) = &mut self.canvas {
                canvas.begin_close_save(budget.graceful_until);
            }
            #[cfg(all(test, windows, feature = "native-composition"))]
            if let Some(probe) = &mut self.input_probe {
                probe.begun = Some(budget.graceful_until);
            }
        }
        let Some(save) = &mut self.close_save else { return Ok(false) };
        save.tighten(budget.graceful_until);
        #[cfg(all(windows, feature = "native-composition"))]
        {
            use sugarsubstitute_application::workspace_session::close_save::SaveVerdict;
            let workspace = self
                .workspace
                .as_ref()
                .map_or(Some(SaveVerdict::Saved), |work| work.close_verdict(now));
            if let Some(verdict) = workspace {
                save.workspace(verdict);
            }
            let inputs = self
                .canvas
                .as_ref()
                .map_or(Some(SaveVerdict::Saved), |canvas| canvas.input_close_verdict(now));
            #[cfg(all(test, windows, feature = "native-composition"))]
            let inputs = self.input_probe.as_ref().map_or(inputs, |probe| probe.verdict);
            if let Some(verdict) = inputs {
                save.inputs(verdict);
            }
        }
        #[cfg(not(all(windows, feature = "native-composition")))]
        {
            use sugarsubstitute_application::workspace_session::close_save::SaveVerdict;
            save.workspace(SaveVerdict::Saved);
            save.inputs(SaveVerdict::Saved);
        }
        save.expire(now);
        if save.settled() {
            return Ok(true);
        }
        let Some(outcome) = save.outcome() else { return Ok(false) };
        if !self.close_record_submitted {
            self.close_record_submitted = true;
            if outcome.saved() {
                save.recorded();
                return Ok(true);
            }
            tracing::warn!(operation = "close_save", outcome = ?outcome,
                "Closing with work that was not saved; recording it for the next start");
            #[cfg(all(windows, feature = "native-composition"))]
            if let Some(work) = &mut self.workspace
                && let Err(cause) = work.submit_close_record(outcome, now, budget.settle_until)
            {
                tracing::error!(operation = "close_save", cause = %cause,
                    "The record of unsaved work could not be submitted");
            }
        }
        #[cfg(all(windows, feature = "native-composition"))]
        let recorded =
            self.workspace.as_mut().is_none_or(workspace::WorkspaceWork::close_record_settled);
        #[cfg(not(all(windows, feature = "native-composition")))]
        let recorded = true;
        let Some(save) = &mut self.close_save else { return Ok(false) };
        if recorded {
            save.recorded();
        }
        Ok(save.settled())
    }
}
