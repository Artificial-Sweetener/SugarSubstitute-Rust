// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! An update failure report offers Copy and Export; nothing restarts and nothing is uploaded.
//! The update owner writes the export through its own execution so this surface never blocks.
use super::{ReportCommand, ReportControl, ReportOwner, shown};
use std::{error::Error, path::PathBuf, sync::Arc};
use sugarsubstitute_contracts::{
    execution::CompletionWake, localization::incident::ReportMessages,
};
use sugarsubstitute_ui::report::{ReportAction, ReportStatus};

/// Writes an update report to a user-selected file away from the window's thread.
pub trait UpdateExport {
    /// Begin with a wake to call when a write completes.
    /// # Errors
    /// Preserves the export owner's start failure.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>>;
    /// Begin writing the report to `destination`. At most one write is outstanding.
    /// # Errors
    /// Preserves admission failures; the report shows them as an export failure.
    fn begin(&mut self, destination: PathBuf) -> Result<(), Box<dyn Error>>;
    /// Take the outcome of the outstanding write once it has finished.
    fn finished(&mut self) -> Option<Result<(), Box<dyn Error>>>;
    /// Stop accepting writes; an outstanding write runs to its outcome.
    fn close(&mut self);
    /// Whether no write is outstanding.
    fn settled(&mut self) -> bool;
}

/// Owner of an update failure report.
pub struct UpdateReport<'a> {
    export: &'a mut dyn UpdateExport,
    words: &'static ReportMessages,
    writing: bool,
}

impl<'a> UpdateReport<'a> {
    /// Report through `export`, with status words from the localization owner.
    pub fn new(export: &'a mut dyn UpdateExport, words: &'static ReportMessages) -> Self {
        Self { export, words, writing: false }
    }

    fn status(&self, success: bool) -> Option<ReportStatus> {
        Some(if success {
            ReportStatus { text: self.words.exported.into(), error: false }
        } else {
            ReportStatus { text: self.words.export_failed.into(), error: true }
        })
    }
}

impl ReportOwner for UpdateReport<'_> {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        self.export.start(wake)
    }
    fn command(
        &mut self,
        command: ReportCommand,
        report: &mut ReportControl<'_>,
    ) -> Result<(), Box<dyn Error>> {
        match command {
            ReportCommand::Export { destination, .. } if !self.writing => {
                match self.export.begin(destination) {
                    Ok(()) => {
                        self.writing = true;
                        report.set_command(ReportAction::Export, shown(false));
                    }
                    Err(cause) => {
                        tracing::error!(operation = "update_report_export", cause = %cause,
                            "Update report export could not start");
                        report.set_status(self.status(false));
                    }
                }
            }
            ReportCommand::Action(ReportAction::Close) => report.close(),
            _ => {}
        }
        Ok(())
    }
    fn poll(&mut self, report: &mut ReportControl<'_>) -> Result<(), Box<dyn Error>> {
        if let Some(result) = self.export.finished() {
            self.writing = false;
            report.set_command(ReportAction::Export, shown(true));
            if let Err(cause) = &result {
                tracing::error!(operation = "update_report_export", cause = %cause,
                    "Update report export failed");
            }
            report.set_status(self.status(result.is_ok()));
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        self.export.close();
        Ok(())
    }
    fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        if self.writing && self.export.finished().is_some() {
            self.writing = false;
        }
        Ok(self.export.settled())
    }
}
