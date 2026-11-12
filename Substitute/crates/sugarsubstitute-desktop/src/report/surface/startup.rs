// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A startup failure report has nothing to decide: Close ends it.
use super::{ReportCommand, ReportControl, ReportOwner};
use std::error::Error;
use sugarsubstitute_ui::report::ReportAction;

/// Owner of a startup failure report.
#[derive(Default)]
pub struct StartupReport;

impl ReportOwner for StartupReport {
    fn command(
        &mut self,
        command: ReportCommand,
        report: &mut ReportControl<'_>,
    ) -> Result<(), Box<dyn Error>> {
        if command == ReportCommand::Action(ReportAction::Close) {
            report.close();
        }
        Ok(())
    }
}
