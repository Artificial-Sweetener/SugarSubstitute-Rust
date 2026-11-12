// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show one Fluent report through the shared native form window. The report owner decides what
//! each command means; this surface owns layout, details, clipboard and export selection.
use crate::installer::native::CopyFailure;
use std::{error::Error, path::PathBuf, sync::Arc};
use sugarsubstitute_contracts::{
    execution::{CompletionWake, CounterDeadline},
    localization::{Messages, incident::ReportMessages},
};
use sugarsubstitute_ui::report::{
    CommandState, ReportAction, ReportContent, ReportKind, ReportLabels, ReportSeverity,
    ReportState, ReportStatus, SummaryRow,
};

mod conflict;
mod crash;
mod lifetime;
#[cfg(all(test, windows))]
#[allow(unsafe_code)]
mod native_tests;
mod startup;
#[cfg(test)]
mod tests;
mod update;

pub use conflict::ConflictReport;
pub use crash::run_report_surface;
pub(super) use crash::validate_content_capacity;
pub use startup::StartupReport;
pub use update::{UpdateExport, UpdateReport};

/// Everything a report shows, in the user's language.
pub struct ReportPresentation {
    pub(super) content: ReportContent,
    pub(super) details: Option<String>,
    pub(super) labels: ReportLabels,
    pub(super) words: &'static ReportMessages,
    pub(super) locale: &'static str,
}

impl ReportPresentation {
    /// Assemble a report from its content and the localization owner's words.
    pub fn new(content: ReportContent, details: Option<String>, copy: &'static Messages) -> Self {
        let words = copy.report;
        let unresponsive = content.kind == ReportKind::UnresponsiveCopy;
        let labels = ReportLabels {
            close_window: words.close_window,
            show_report: words.show_report,
            hide_report: words.hide_report,
            copy: words.copy,
            export: words.export,
            close: if unresponsive { copy.conflict.close } else { words.close },
            restart: words.restart,
            cancel: copy.conflict.cancel,
            include_dump: words.include_dump,
            dump_warning: words.dump_warning,
        };
        let content = ReportContent { has_details: details.is_some(), ..content };
        Self { content, details, labels, words, locale: copy.id }
    }

    /// A launch that could not start: heading, explanation and the redacted cause chain.
    pub fn startup_failure(
        copy: &'static Messages,
        window_title: &str,
        summary: &str,
        details: String,
    ) -> Self {
        Self::new(
            ReportContent {
                kind: ReportKind::StartupFailure,
                severity: ReportSeverity::Error,
                window_title: window_title.to_owned(),
                title: summary.to_owned(),
                message: copy.report.startup_message.to_owned(),
                summary: Vec::new(),
                has_details: true,
            },
            Some(details),
            copy,
        )
    }

    /// A failed update attempt: version and step as summary facts, then the full report.
    pub fn update_failure(
        copy: &'static Messages,
        window_title: &str,
        version: &str,
        step: &str,
        details: String,
    ) -> Self {
        let summary = vec![
            SummaryRow { label: copy.report.version.to_owned(), value: version.to_owned() },
            SummaryRow { label: copy.report.update_phase.to_owned(), value: step.to_owned() },
        ];
        Self::new(
            ReportContent {
                kind: ReportKind::UpdateFailure,
                severity: ReportSeverity::Warning,
                window_title: window_title.to_owned(),
                title: copy.report.update_title.to_owned(),
                message: copy.report.update_message.to_owned(),
                summary,
                has_details: true,
            },
            Some(details),
            copy,
        )
    }

    /// The question asked when the previous SugarSubstitute window has stopped responding.
    pub fn unresponsive_copy(copy: &'static Messages, window_title: &str) -> Self {
        Self::new(
            ReportContent {
                kind: ReportKind::UnresponsiveCopy,
                severity: ReportSeverity::Warning,
                window_title: window_title.to_owned(),
                title: copy.conflict.title.to_owned(),
                message: copy.conflict.prompt.to_owned(),
                summary: Vec::new(),
                has_details: false,
            },
            None,
            copy,
        )
    }
}

/// A command the user chose that the report's owner decides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReportCommand {
    /// Close, Restart, Force or Cancel.
    Action(ReportAction),
    /// Write the report to the destination the user selected.
    Export {
        /// Selected destination; the export owner validates it.
        destination: PathBuf,
        /// Explicit dump consent at the time of selection.
        include_dump: bool,
    },
}

/// The owner's view of the open report: it may change the message, commands, status and full
/// report text, copy that text, and close the report. Every change repaints.
pub struct ReportControl<'a> {
    pub(super) message: &'a mut String,
    pub(super) state: &'a mut ReportState,
    pub(super) closed: &'a mut bool,
    pub(super) changed: &'a mut bool,
    pub(super) requests: &'a mut ControlRequests,
}

/// Owner requests the report applies after the owner's turn.
#[derive(Default)]
pub(super) struct ControlRequests {
    /// Replacement for the full report text.
    pub(super) details: Option<String>,
    /// Copy the full report text, optionally only before this counter deadline.
    pub(super) copy: Option<Option<CounterDeadline>>,
}

impl ReportControl<'_> {
    /// Replace the explanation below the heading.
    pub fn set_message(&mut self, message: &str) {
        if self.message != message {
            message.clone_into(self.message);
            *self.changed = true;
        }
    }
    /// Show, hide, enable or disable one command.
    pub fn set_command(&mut self, action: ReportAction, command: CommandState) {
        if self.state.command(action) != command {
            self.state.set(action, command);
            *self.changed = true;
        }
    }
    /// Replace the footer status line.
    pub fn set_status(&mut self, status: Option<ReportStatus>) {
        if self.state.status != status {
            self.state.status = status;
            *self.changed = true;
        }
    }
    /// End the report once the owner has nothing left to do.
    pub fn close(&mut self) {
        *self.closed = true;
    }
    /// Replace the full report text; dump consent resets with it.
    pub fn replace_details(&mut self, details: String) {
        self.requests.details = Some(details);
        *self.changed = true;
    }
    /// Copy the full report text now, or only while `before` has not passed.
    pub fn copy_details(&mut self, before: Option<CounterDeadline>) {
        self.requests.copy = Some(before);
    }
}

/// The application side of one report. Calls arrive on the window's thread between native
/// callbacks and must return promptly; longer work belongs to the owner's own execution.
pub trait ReportOwner {
    /// Begin with a wake that schedules another turn when owner work completes.
    /// # Errors
    /// Preserves the owner's start failure; the report then closes.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        let _ = wake;
        Ok(())
    }
    /// Decide a user command. Details and dump consent never reach the owner; Copy and Export
    /// reach it through [`ReportOwner::copy`] and [`ReportOwner::export_requested`].
    /// # Errors
    /// Preserves the owner's failure; the report then closes.
    fn command(
        &mut self,
        command: ReportCommand,
        report: &mut ReportControl<'_>,
    ) -> Result<(), Box<dyn Error>>;
    /// Apply progress since the last turn.
    /// # Errors
    /// Preserves the owner's failure; the report then closes.
    fn poll(&mut self, report: &mut ReportControl<'_>) -> Result<(), Box<dyn Error>> {
        let _ = report;
        Ok(())
    }
    /// The user chose Copy. `Ok(true)` copies the full report at once; an owner that must
    /// authorize the copy first returns `Ok(false)` and later calls
    /// [`ReportControl::copy_details`].
    /// # Errors
    /// Preserves the owner's failure; the report then closes.
    fn copy(&mut self, report: &mut ReportControl<'_>) -> Result<bool, Box<dyn Error>> {
        let _ = report;
        Ok(true)
    }
    /// Observe a finished clipboard transfer; the report already shows its outcome.
    /// # Errors
    /// Preserves the owner's failure; the report then closes.
    fn copied(
        &mut self,
        outcome: Result<(), CopyFailure>,
        report: &mut ReportControl<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let _ = (outcome, report);
        Ok(())
    }
    /// The user chose Export with the current dump consent. `Ok(true)` asks for a destination.
    /// # Errors
    /// Preserves the owner's failure; the report then closes.
    fn export_requested(
        &mut self,
        include_dump: bool,
        report: &mut ReportControl<'_>,
    ) -> Result<bool, Box<dyn Error>> {
        let _ = (include_dump, report);
        Ok(true)
    }
    /// The destination picker was dismissed or failed; no export started.
    /// # Errors
    /// Preserves the owner's failure; the report then closes.
    fn export_abandoned(&mut self, report: &mut ReportControl<'_>) -> Result<(), Box<dyn Error>> {
        let _ = report;
        Ok(())
    }
    /// A frame showing full report text revision `details_revision` was presented natively.
    /// The original text is revision zero; each [`ReportControl::replace_details`] adds one.
    /// # Errors
    /// Preserves the owner's failure; the report then closes.
    fn presented(&mut self, details_revision: u64) -> Result<(), Box<dyn Error>> {
        let _ = details_revision;
        Ok(())
    }
    /// The window is closing (the owner's own close, or a native or rendering failure).
    /// Seal new work.
    /// # Errors
    /// Preserves the failure while settlement continues.
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    /// Whether the closing window may retire before [`ReportOwner::settled`].
    /// # Errors
    /// Preserves unresolved cleanup.
    fn window_releasable(&mut self) -> Result<bool, Box<dyn Error>> {
        self.settled()
    }
    /// The window has actually been destroyed; settlement continues without it.
    /// # Errors
    /// Preserves the owner's failure.
    fn window_retired(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    /// Whether the owner's work has actually retired.
    /// # Errors
    /// Preserves unresolved cleanup.
    fn settled(&mut self) -> Result<bool, Box<dyn Error>> {
        Ok(true)
    }
    /// Release final ownership after the window and the event loop have retired.
    /// # Errors
    /// Preserves final cleanup failures.
    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

/// Show `presentation` until its owner closes it and has settled.
/// Must run on the main thread after any previous native window has retired, never from inside
/// another window's event callback.
/// # Errors
/// Preserves native window, rendering and owner failures.
pub fn show_report(
    presentation: ReportPresentation,
    owner: &mut dyn ReportOwner,
) -> Result<(), Box<dyn Error>> {
    let window = report_window(presentation.content.window_title.clone());
    let mut lifetime = lifetime::ReportLifetime::new(presentation, owner)?;
    crate::installer::native::run(window, &mut lifetime)
}

/// The report's window: fixed width, sized to its content once the first frame is measured.
fn report_window(title: String) -> crate::installer::native::Window {
    crate::installer::native::Window {
        title,
        visible: true,
        close: ReportAction::Dismiss.id(),
        size: [sugarsubstitute_ui::report::WIDTH.into(), 320.0],
        min_size: None,
        resizable: false,
    }
}

/// Command availability shorthand.
pub(super) const fn shown(enabled: bool) -> CommandState {
    CommandState { visible: true, enabled }
}
