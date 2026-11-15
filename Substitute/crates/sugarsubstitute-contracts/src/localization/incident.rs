// SPDX-License-Identifier: AGPL-3.0-or-later
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

/// Placeholder in [`ReportMessages::update_file_name`] for the product the report is about,
/// followed by the version of its update when that is known.
pub const PRODUCT: &str = "{product}";
/// Placeholder in [`ReportMessages::update_file_name`] for the local date, as `YYYY-MM-DD`.
pub const DATE: &str = "{date}";
/// Placeholder in [`ReportMessages::update_file_name`] for the local time, as `HH-MM`.
pub const TIME: &str = "{time}";

/// A calendar date and time of day in the user's time zone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalDateTime {
    /// Four-digit year.
    pub year: u16,
    /// Month, 1 to 12.
    pub month: u8,
    /// Day of the month, 1 to 31.
    pub day: u8,
    /// Hour, 0 to 23.
    pub hour: u8,
    /// Minute, 0 to 59.
    pub minute: u8,
}

/// `template` with [`PRODUCT`] replaced by `product`, [`DATE`] by `now`'s date as `YYYY-MM-DD`
/// and [`TIME`] by its time as `HH-MM`; a name without placeholders is returned unchanged.
pub fn file_name(template: &str, product: &str, now: LocalDateTime) -> String {
    let date = format!("{:04}-{:02}-{:02}", now.year, now.month, now.day);
    let time = format!("{:02}-{:02}", now.hour, now.minute);
    template.replace(PRODUCT, product).replace(DATE, &date).replace(TIME, &time)
}

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
    /// Title of the dialog that saves a crash or start-up report.
    pub save_title: &'static str,
    /// Title of the dialog that exports an update report.
    pub update_save_title: &'static str,
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
    /// Suggested file name when saving an update report; contains [`PRODUCT`], [`DATE`] and [`TIME`].
    pub update_file_name: &'static str,
    /// Heading of a report on a SugarCubes update that failed during start-up.
    pub pack_update_title: &'static str,
    /// Explain that SugarCubes was not updated while the version installed keeps working.
    pub pack_update_message: &'static str,
    /// Accepted startup failure categories in the explicit StartupFailureCode order.
    pub startup_failure: [&'static str; 5],
}

impl ReportMessages {
    /// Borrow every owned label for complete-coverage validation.
    pub fn labels(&self) -> [&'static str; 72] {
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
            self.update_save_title,
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
            self.update_file_name,
            self.pack_update_title,
            self.pack_update_message,
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
    use super::{EN, LocalDateTime, file_name};

    #[test]
    fn a_saved_update_report_is_named_for_its_product_and_date_in_every_language() {
        let date = LocalDateTime { year: 2026, month: 11, day: 4, hour: 9, minute: 5 };
        for messages in crate::localization::LOCALES {
            let words = messages.report;
            assert!(!words.save_title.contains("incident"), "{}", messages.id);
            assert_ne!(words.update_save_title, words.save_title, "{}", messages.id);
            let name = file_name(words.update_file_name, "SugarCubes 0.15.6", date);
            assert!(
                name.contains("SugarCubes 0.15.6")
                    && name.contains("2026-11-04")
                    && name.contains("09-05"),
                "{name}"
            );
            assert!(!name.contains('{') && name.ends_with(".txt"), "{name}");
        }
        assert_eq!(
            file_name(EN.update_file_name, "SugarCubes", date),
            "SugarCubes update report 2026-11-04 09-05.txt"
        );
        assert_eq!(
            file_name(EN.update_file_name, "SugarCubes 0.15.6", date),
            "SugarCubes 0.15.6 update report 2026-11-04 09-05.txt"
        );
        assert_eq!(EN.save_title, "Save the report");
        assert_eq!(EN.update_save_title, "Export update report");
        assert_eq!(file_name("report.txt", "SugarCubes", date), "report.txt");
    }
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
