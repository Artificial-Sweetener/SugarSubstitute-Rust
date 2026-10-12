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
pub struct ReportMessages {
    /// Native incident window title.
    pub title: &'static str,
    /// Locally stored incident explanation, independent of failure classification.
    pub explanation: &'static str,
    /// Copy redacted text action.
    pub copy: &'static str,
    /// Select local export action.
    pub export: &'static str,
    /// Durable dismissal action.
    pub close: &'static str,
    /// Dismiss and request fresh restart action.
    pub restart: &'static str,
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
    /// Confirm destination selection action.
    pub save: &'static str,
    /// Cancel destination selection action.
    pub cancel: &'static str,
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
    /// Accepted startup failure mechanism.
    pub startup_failed: &'static str,
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
    /// Accepted startup failure categories in the explicit StartupFailureCode order.
    pub startup_failure: [&'static str; 5],
}

impl ReportMessages {
    /// Borrow every owned label for complete-coverage validation.
    pub fn labels(&self) -> [&'static str; 57] {
        [
            self.title,
            self.explanation,
            self.copy,
            self.export,
            self.close,
            self.restart,
            self.include_dump,
            self.dump_warning,
            self.save_title,
            self.text_filter,
            self.archive_filter,
            self.save,
            self.cancel,
            self.copied,
            self.exported,
            self.copy_failed,
            self.export_failed,
            self.ack_failed,
            self.unexplained,
            self.startup_failed,
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
