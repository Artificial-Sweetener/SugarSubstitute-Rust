// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A SugarCubes update that failed during start-up, as the window's update report shows it.
//!
//! The host that keeps SugarCubes' own update report turns a failed one into this value: the
//! version it tried to install, the failed step and the report's plain lines, both already in
//! the user's language from the owner of SugarCubes update words, and the diagnostic codes worth
//! handing over. Every piece is redacted and bounded here, so whatever the host passes, the
//! window never shows or exports a path, an address or a credential.
use super::{
    attempt::{MAX_CAUSES, MAX_VERSION_BYTES, sanitize},
    pack::{PackOutcome, PackUpdateReport},
};
use crate::localization::pack_updates::PackUpdateMessages;

/// A failed SugarCubes start-up update; the installed version keeps running.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackUpdateFailure {
    version: Option<String>,
    step: Option<String>,
    lines: Vec<String>,
    causes: Vec<String>,
}

impl PackUpdateFailure {
    /// Describe a failed update. `version` is the version the update tried to install, when
    /// known; `step` names the failed step and `lines` are the report's plain lines, both in the
    /// user's language; `causes` are diagnostic facts such as a refusal code. Each piece is
    /// redacted, empty pieces are dropped, and at most `MAX_CAUSES` lines and causes remain.
    pub fn new<L, C>(version: Option<&str>, step: Option<&str>, lines: L, causes: C) -> Self
    where
        L: IntoIterator,
        L::Item: AsRef<str>,
        C: IntoIterator,
        C::Item: AsRef<str>,
    {
        // A prefix of redacted text is still redacted, so truncating after sanitizing is safe.
        let one = |text: &str, maximum: usize| {
            let text = truncate(&sanitize(text), maximum);
            (!text.is_empty()).then_some(text)
        };
        Self {
            version: version.and_then(|version| one(version.trim(), MAX_VERSION_BYTES)),
            step: step.and_then(|step| one(step, usize::MAX)),
            lines: bounded_list(lines),
            causes: bounded_list(causes),
        }
    }

    /// The failure a SugarCubes update report describes, in `words`; `None` exactly when the
    /// report does not [need attention](PackUpdateReport::needs_attention). A failed update
    /// names the version it tried and its failed step, a restore names the version it removed,
    /// and a check that reached no release source, or stopped before it changed anything, names
    /// the checking step. The step's own report line is left out, since the step is
    /// shown on its own.
    pub fn from_report(report: &PackUpdateReport, words: &PackUpdateMessages) -> Option<Self> {
        let (version, step, causes) = match report.outcome {
            PackOutcome::Failed => {
                let step = match report.step.as_deref() {
                    Some("downloading") => 1,
                    Some("installing") => 2,
                    _ => 0,
                };
                (report.to.as_deref(), Some(words.steps[step]), vec![&report.code, &report.reason])
            }
            // The report's first line says in plain words why the previous version is back, so the
            // reason's code adds nothing for the reader.
            PackOutcome::Restored => {
                let step = match report.reason.as_deref() {
                    Some("interrupted") => Some(words.steps[2]),
                    Some("did-not-start" | "page-did-not-load") => Some(words.steps[3]),
                    _ => None,
                };
                (report.from.as_deref(), step, Vec::new())
            }
            PackOutcome::CouldNotCheck | PackOutcome::Stopped => {
                (None, Some(words.steps[0]), Vec::new())
            }
            PackOutcome::Updated
            | PackOutcome::UpToDate
            | PackOutcome::Judging
            | PackOutcome::NotManaged
            | PackOutcome::Busy
            | PackOutcome::Available => return None,
        };
        let lines = report.lines(words).into_iter().filter(|line| Some(line.as_str()) != step);
        Some(Self::new(version, step, lines, causes.into_iter().flatten()))
    }

    /// The version the update tried to install, when known.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
    /// The failed step in the user's language, when known.
    pub fn step(&self) -> Option<&str> {
        self.step.as_deref()
    }
    /// The report's plain lines in the user's language, the outcome first.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }
    /// Diagnostic facts worth handing over, redacted.
    pub fn causes(&self) -> &[String] {
        &self.causes
    }
}

/// Redact each item, drop empty ones and keep at most `MAX_CAUSES`.
fn bounded_list<I>(items: I) -> Vec<String>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    items
        .into_iter()
        .map(|item| sanitize(item.as_ref()))
        .filter(|item| !item.is_empty())
        .take(MAX_CAUSES)
        .collect()
}

/// Truncate at a character boundary within `maximum` bytes.
fn truncate(text: &str, maximum: usize) -> String {
    let mut end = text.len().min(maximum);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_keeps_its_plain_words_and_drops_paths_addresses_and_credentials() {
        let failure = PackUpdateFailure::new(
            Some(" 0.16.0 "),
            Some("Downloading the update"),
            ["SugarCubes couldn't be updated.", "", r"C:\Users\Jo Ann\ComfyUI\custom_nodes"],
            ["digest-mismatch", "https://api.comfy.org/nodes?token=abc", "token=abc"],
        );
        assert_eq!(failure.version(), Some("0.16.0"));
        assert_eq!(failure.step(), Some("Downloading the update"));
        assert_eq!(failure.lines(), ["SugarCubes couldn't be updated.", "<path>"]);
        assert_eq!(failure.causes(), ["digest-mismatch", "<address>", "<redacted>"]);
    }

    #[test]
    fn a_failed_restored_or_unchecked_report_becomes_the_failure_the_window_shows() {
        let words = &crate::localization::pack_updates::EN;
        let failed = PackUpdateReport {
            from: Some("0.15.0".into()),
            to: Some("0.16.0".into()),
            step: Some("installing".into()),
            code: Some("digest-mismatch".into()),
            unchecked: vec!["github-release".into()],
            ..PackUpdateReport::new(PackOutcome::Failed)
        };
        assert_eq!(
            PackUpdateFailure::from_report(&failed, words),
            Some(PackUpdateFailure::new(
                Some("0.16.0"),
                Some(words.steps[2]),
                [words.could_not_update, words.github_unchecked],
                ["digest-mismatch"],
            ))
        );
        let restored = PackUpdateReport {
            from: Some("0.16.0".into()),
            to: Some("0.15.0".into()),
            reason: Some("did-not-start".into()),
            ..PackUpdateReport::new(PackOutcome::Restored)
        };
        assert_eq!(
            PackUpdateFailure::from_report(&restored, words),
            Some(PackUpdateFailure::new(
                Some("0.16.0"),
                Some(words.steps[3]),
                [words.restored_did_not_start],
                [""; 0],
            )),
            "a put-back says why in plain words and shows no raw code"
        );
        for (reason, step, line) in [
            ("page-did-not-load", Some(words.steps[3]), words.restored_page),
            ("interrupted", Some(words.steps[2]), words.restored_interrupted),
            ("unknown-reason", None, words.previous_running),
        ] {
            let restored = PackUpdateReport { reason: Some(reason.into()), ..restored.clone() };
            let failure = PackUpdateFailure::from_report(&restored, words);
            assert_eq!(failure.as_ref().and_then(PackUpdateFailure::step), step, "{reason}");
            assert_eq!(
                failure.as_ref().map(PackUpdateFailure::lines),
                Some(&[line.to_owned()][..])
            );
            assert_eq!(failure.as_ref().map(|failure| failure.causes().len()), Some(0));
        }
        let unchecked = PackUpdateReport {
            unchecked: vec!["registry".into(), "github-release".into()],
            ..PackUpdateReport::new(PackOutcome::CouldNotCheck)
        };
        assert_eq!(
            PackUpdateFailure::from_report(&unchecked, words),
            Some(PackUpdateFailure::new(
                None,
                Some(words.steps[0]),
                [words.could_not_check, words.registry_unchecked, words.github_unchecked],
                [""; 0],
            ))
        );
    }

    #[test]
    fn a_report_that_is_not_a_failure_shows_no_failure() {
        let words = &crate::localization::pack_updates::EN;
        for outcome in [
            PackOutcome::Updated,
            PackOutcome::UpToDate,
            PackOutcome::Judging,
            PackOutcome::NotManaged,
            PackOutcome::Busy,
            PackOutcome::Available,
        ] {
            let report =
                PackUpdateReport { to: Some("0.16.0".into()), ..PackUpdateReport::new(outcome) };
            assert_eq!(PackUpdateFailure::from_report(&report, words), None, "{outcome:?}");
        }
    }

    /// The window's warning and its report agree: every report that needs attention becomes a
    /// failure the report shows, and no other does. A stopped update tells the user it couldn't
    /// check, at the checking step.
    #[test]
    fn exactly_the_reports_that_need_attention_become_failures() {
        let words = &crate::localization::pack_updates::EN;
        for outcome in [
            PackOutcome::Updated,
            PackOutcome::UpToDate,
            PackOutcome::Restored,
            PackOutcome::Judging,
            PackOutcome::NotManaged,
            PackOutcome::CouldNotCheck,
            PackOutcome::Busy,
            PackOutcome::Failed,
            PackOutcome::Stopped,
            PackOutcome::Available,
        ] {
            let report = PackUpdateReport::new(outcome);
            let failure = PackUpdateFailure::from_report(&report, words);
            assert_eq!(failure.is_some(), report.needs_attention(), "{outcome:?}");
        }
        assert_eq!(
            PackUpdateFailure::from_report(&PackUpdateReport::new(PackOutcome::Stopped), words),
            Some(PackUpdateFailure::new(
                None,
                Some(words.steps[0]),
                [words.could_not_check],
                [""; 0]
            ))
        );
    }

    #[test]
    fn a_failure_is_bounded_however_much_the_host_passes() {
        let many = vec!["line"; 40];
        let failure = PackUpdateFailure::new(Some(&"1.".repeat(100)), Some(""), many.clone(), many);
        assert_eq!(failure.version().map(str::len), Some(MAX_VERSION_BYTES));
        assert_eq!(failure.step(), None);
        assert_eq!(failure.lines().len(), MAX_CAUSES);
        assert_eq!(failure.causes().len(), MAX_CAUSES);
    }
}
