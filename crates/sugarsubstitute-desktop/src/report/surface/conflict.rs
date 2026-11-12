// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Ask whether to close an unresponsive previous SugarSubstitute window and start. The
//! conflict controller owns process authority, waiting and timing; this adapter only offers
//! "Close it and start" while the controller can act, and shows plain progress meanwhile.
use super::{ReportCommand, ReportControl, ReportOwner, shown};
use std::{error::Error, sync::Arc};
use sugarsubstitute_contracts::{
    execution::CompletionWake,
    instance_conflict::{ConflictAction, ConflictController, ConflictPhase},
    localization::conflict::ConflictMessages,
};
use sugarsubstitute_ui::report::{ReportAction, ReportStatus};

/// Owner adapter for a [`ConflictController`].
pub struct ConflictReport<'a> {
    controller: &'a mut dyn ConflictController,
    words: &'static ConflictMessages,
    /// The controller allows ending the previous window outright rather than asking it to close.
    forceable: bool,
    done: bool,
    cancelled: bool,
}

impl<'a> ConflictReport<'a> {
    /// Mirror `controller` until it reports that its work has retired.
    pub fn new(
        controller: &'a mut dyn ConflictController,
        words: &'static ConflictMessages,
    ) -> Self {
        Self { controller, words, forceable: false, done: false, cancelled: false }
    }

    /// The question and the progress line this surface shows for `phase`, in its own locale.
    fn words_for(&self, phase: ConflictPhase) -> (&'static str, Option<&'static str>) {
        let words = self.words;
        match phase {
            ConflictPhase::Checking => (words.prompt, Some(words.checking)),
            ConflictPhase::AwaitingDecision | ConflictPhase::Done => (words.prompt, None),
            ConflictPhase::Closing | ConflictPhase::Forcing => (words.prompt, Some(words.waiting)),
            ConflictPhase::ForceOffered => (words.force_prompt, None),
            ConflictPhase::Failed => (words.failed, None),
            ConflictPhase::Retiring { continuing: true } => (words.prompt, Some(words.cleanup)),
            ConflictPhase::Retiring { continuing: false } => (words.prompt, None),
        }
    }
}

impl ReportOwner for ConflictReport<'_> {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        self.controller.start(wake);
        Ok(())
    }
    fn command(
        &mut self,
        command: ReportCommand,
        _: &mut ReportControl<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let action = match command {
            ReportCommand::Action(ReportAction::Close) if self.forceable => ConflictAction::Force,
            ReportCommand::Action(ReportAction::Close) => ConflictAction::Close,
            ReportCommand::Action(ReportAction::Cancel) => ConflictAction::Cancel,
            _ => return Ok(()),
        };
        self.cancelled |= action == ConflictAction::Cancel;
        self.controller.choose(action);
        Ok(())
    }
    fn poll(&mut self, report: &mut ReportControl<'_>) -> Result<(), Box<dyn Error>> {
        let view = self.controller.poll();
        self.forceable = view.force_enabled();
        let actionable = (view.close_enabled() || view.force_enabled()) && !self.cancelled;
        report.set_command(ReportAction::Close, shown(actionable));
        report.set_command(ReportAction::Cancel, shown(!view.done() && !self.cancelled));
        let (message, progress) = self.words_for(view.phase);
        report.set_message(message);
        report.set_status(progress.map(|text| ReportStatus { text: text.into(), error: false }));
        self.done = view.done();
        if self.done {
            report.close();
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.done && !self.cancelled {
            self.cancelled = true;
            self.controller.choose(ConflictAction::Cancel);
        }
        Ok(())
    }
    fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.done {
            self.done = self.controller.poll().done();
        }
        Ok(self.done)
    }
}
