// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Complete incident presentation copy selected by the sole supported-locale registry.

mod en;
pub use en::EN;
mod zh_hans;
pub use zh_hans::ZH_HANS;
mod ja;
pub use ja::JA;
mod ko;
pub use ko::KO;
mod es;
pub use es::ES;

/// Bind localized native report controls, status and evidence labels.
#[non_exhaustive]
pub struct ReportMessages {
    /// Native incident window title.
    pub title: &'static str,
    /// Locally stored incident explanation, independent of failure classification.
    pub explanation: &'static str,
    /// Explicit existing coordinator/client coverage after fresh-supervisor recovery.
    pub recovery_capture_degraded: &'static str,
    /// Copy redacted text action.
    pub copy: &'static str,
    /// Select local export action.
    pub export: &'static str,
    /// Durable dismissal action.
    pub close: &'static str,
    /// Dismiss and request fresh restart action.
    pub restart: &'static str,
    /// Restart is unavailable; explicit close and normal reopening remain supported.
    pub restart_unavailable: &'static str,
    /// Explicit native memory inclusion consent.
    pub include_dump: &'static str,
    /// Unsanitized sensitive native memory disclosure.
    pub dump_warning: &'static str,
    /// Native destination selection title.
    pub save_title: &'static str,
    /// Text export format label.
    pub text_filter: &'static str,
    /// Dump archive format label.
    pub archive_filter: &'static str,
    /// Successful clipboard transfer status.
    pub copied: &'static str,
    /// Successful export status.
    pub exported: &'static str,
    /// Failed clipboard transfer status.
    pub copy_failed: &'static str,
    /// Failed export status.
    pub export_failed: &'static str,
    /// Failed durable acknowledgement status.
    pub ack_failed: &'static str,
    /// Unexplained termination mechanism.
    pub unexplained: &'static str,
    /// An authenticated host error was retained independently of native crash capture.
    pub host_failed: &'static str,
    /// Accepted startup failure mechanism.
    pub startup_failed: &'static str,
    /// Startup cannot continue because the installation was changed by a newer release in a way
    /// this release cannot read; plain words that reinstalling keeps the user's files.
    pub reinstall_needed: &'static str,
    /// Observed Rust panic mechanism.
    pub panic: &'static str,
    /// Observed abort mechanism.
    pub abort: &'static str,
    /// Observed native exception mechanism.
    pub native_fault: &'static str,
    /// Explicit unavailable evidence status.
    pub unavailable: &'static str,
    /// Evidence or operation remains pending.
    pub pending: &'static str,
    /// Retained payload disposition.
    pub retained: &'static str,
    /// Discarded payload disposition.
    pub discarded: &'static str,
    /// Bounded text omission marker.
    pub omitted: &'static str,
    /// Build identity label.
    pub build: &'static str,
    /// Committed summary revision label.
    pub revision: &'static str,
    /// Accepted native exit status label.
    pub exit_code: &'static str,
    /// Process exit was not observed; native fault evidence remains independent.
    pub exit_unobserved: &'static str,
    /// Capture availability label.
    pub capture: &'static str,
    /// Actual failing frame label.
    pub symbols: &'static str,
    /// Redacted diagnostic tail label.
    pub diagnostics: &'static str,
    /// Summary eviction count label.
    pub evictions: &'static str,
    /// Quarantined payload disposition, independent of ordinary unavailability.
    pub quarantined: &'static str,
    /// Capture unavailability in the explicit CaptureAbsence declaration order.
    pub capture_absence: [&'static str; 8],
    /// Resolution failure in the explicit SymbolFailure declaration order.
    pub symbol_failure: [&'static str; 8],
    /// Accessible name of the report window close control.
    pub close_window: &'static str,
    /// Reveal the complete report text.
    pub show_report: &'static str,
    /// Collapse the complete report text.
    pub hide_report: &'static str,
    /// Explain a startup failure report.
    pub startup_message: &'static str,
    /// Heading of an update failure report.
    pub update_title: &'static str,
    /// Explain that an update failed while the installed version remains usable.
    pub update_message: &'static str,
    /// Version summary label.
    pub version: &'static str,
    /// Update step summary label.
    pub update_phase: &'static str,
    /// Suggested file name when exporting an update report.
    pub export_file_name: &'static str,
    /// Accepted startup failure categories in the explicit StartupFailureCode order.
    pub startup_failure: [&'static str; 5],
}

impl ReportMessages {
    /// Borrow every owned label for complete-coverage validation.
    pub fn labels(&self) -> [&'static str; 69] {
        [
            self.title,
            self.explanation,
            self.recovery_capture_degraded,
            self.copy,
            self.export,
            self.close,
            self.restart,
            self.restart_unavailable,
            self.include_dump,
            self.dump_warning,
            self.save_title,
            self.text_filter,
            self.archive_filter,
            self.copied,
            self.exported,
            self.copy_failed,
            self.export_failed,
            self.ack_failed,
            self.unexplained,
            self.host_failed,
            self.startup_failed,
            self.reinstall_needed,
            self.panic,
            self.abort,
            self.native_fault,
            self.unavailable,
            self.pending,
            self.retained,
            self.discarded,
            self.omitted,
            self.build,
            self.revision,
            self.exit_code,
            self.exit_unobserved,
            self.capture,
            self.symbols,
            self.diagnostics,
            self.evictions,
            self.quarantined,
            self.capture_absence[0],
            self.capture_absence[1],
            self.capture_absence[2],
            self.capture_absence[3],
            self.capture_absence[4],
            self.capture_absence[5],
            self.capture_absence[6],
            self.capture_absence[7],
            self.symbol_failure[0],
            self.symbol_failure[1],
            self.symbol_failure[2],
            self.symbol_failure[3],
            self.symbol_failure[4],
            self.symbol_failure[5],
            self.symbol_failure[6],
            self.symbol_failure[7],
            self.close_window,
            self.show_report,
            self.hide_report,
            self.startup_message,
            self.update_title,
            self.update_message,
            self.version,
            self.update_phase,
            self.export_file_name,
            self.startup_failure[0],
            self.startup_failure[1],
            self.startup_failure[2],
            self.startup_failure[3],
            self.startup_failure[4],
        ]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_registered_locale_has_complete_incident_copy() {
        for locale in super::super::LOCALES {
            assert!(
                locale
                    .report
                    .labels()
                    .iter()
                    .all(|label| !label.is_empty() && !label.contains('\0'))
            );
        }
        assert!(std::ptr::eq(super::super::messages("unknown").report, &super::EN));
    }
}
