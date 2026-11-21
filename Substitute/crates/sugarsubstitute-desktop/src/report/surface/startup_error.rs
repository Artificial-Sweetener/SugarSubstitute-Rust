// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A failed start retrying by itself cannot fix: the dialog explains it and records Retry or
//! Close for the application, which acts on the decision.
use super::{ReportCommand, ReportControl, ReportOwner, shown};
use std::error::Error;
use sugarsubstitute_contracts::startup::{StartupErrorAnswer, StartupErrorDecision};
use sugarsubstitute_ui::report::ReportAction;

/// Owner of a startup error dialog.
pub struct StartupErrorReport {
    answer: StartupErrorAnswer,
    answered: bool,
}

impl StartupErrorReport {
    /// Record the dialog's decision in `answer`.
    pub fn new(answer: StartupErrorAnswer) -> Self {
        Self { answer, answered: false }
    }

    /// Record `decision` once and close the dialog.
    fn decide(&mut self, decision: StartupErrorDecision, report: &mut ReportControl<'_>) {
        if !self.answered {
            self.answered = true;
            self.answer.answer(decision);
        }
        report.close();
    }
}

impl ReportOwner for StartupErrorReport {
    fn command(
        &mut self,
        command: ReportCommand,
        report: &mut ReportControl<'_>,
    ) -> Result<(), Box<dyn Error>> {
        match command {
            ReportCommand::Action(ReportAction::Close) => {
                self.decide(StartupErrorDecision::Retry, report)
            }
            ReportCommand::Action(ReportAction::Cancel) => {
                self.decide(StartupErrorDecision::Close, report)
            }
            _ => {}
        }
        Ok(())
    }
    fn poll(&mut self, report: &mut ReportControl<'_>) -> Result<(), Box<dyn Error>> {
        report.set_command(ReportAction::Close, shown(!self.answered));
        report.set_command(ReportAction::Cancel, shown(!self.answered));
        Ok(())
    }
    /// Closing the dialog's window closes the application, as its Close button does.
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.answered {
            self.answered = true;
            self.answer.answer(StartupErrorDecision::Close);
        }
        Ok(())
    }
}
