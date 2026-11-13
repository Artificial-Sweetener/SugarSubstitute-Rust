// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! What the host tells the user about the SugarCubes update during start-up: the splash line
//! while it runs, the one plain line after it, and its update report.
//!
//! [`PackUpdateReport`] is the update report the coordinator keeps as
//! `state/pack-updates/SugarCubes/report.json`; this module owns its bytes, so the writer and
//! every reader (the window's update report) agree. [`PackUpdateStatus`] is the live state a
//! coordinator publishes to its window while the update runs.
use crate::localization::{pack_updates::PackUpdateMessages, updates::UpdateMessages};
use serde::{Deserialize, Serialize};

/// The only supported report schema.
pub const PACK_REPORT_SCHEMA: u32 = 1;
/// The largest report read.
pub const MAX_PACK_REPORT_BYTES: usize = 16 * 1024;

/// What one SugarCubes startup update ended with; spelled as in the report.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackOutcome {
    /// A newer version was installed.
    Updated,
    /// Nothing newer is published.
    UpToDate,
    /// The previous version was installed again after the new one failed.
    Restored,
    /// An earlier update is still being judged.
    Judging,
    /// The folder is left alone.
    NotManaged,
    /// No release source answered usefully.
    CouldNotCheck,
    /// SugarCubes' own updater was changing the folder.
    Busy,
    /// A step failed; the installed version runs.
    Failed,
    /// The update was cancelled or ran out of time before changing anything.
    Stopped,
    /// A newer version exists for a ComfyUI the host does not start.
    Available,
}

/// The update report of one SugarCubes startup update.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackUpdateReport {
    /// Always [`PACK_REPORT_SCHEMA`].
    pub schema: u32,
    /// What happened.
    pub outcome: PackOutcome,
    /// The version replaced, or the one that failed and was removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// The version now installed, available or being judged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// `registry` or `github-release` for an installed version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Release sources that did not answer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unchecked: Vec<String>,
    /// Why the folder is left alone, or why a version was restored; shared spellings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The step that failed: `checking`, `downloading` or `installing`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    /// The failure's shared refusal code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// A report that cannot be read; a reader treats it as no report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackReportError;

impl std::fmt::Display for PackReportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("The SugarCubes update report is unreadable")
    }
}

impl std::error::Error for PackReportError {}

impl PackUpdateReport {
    /// A report of `outcome` with no further facts.
    pub fn new(outcome: PackOutcome) -> Self {
        Self {
            schema: PACK_REPORT_SCHEMA,
            outcome,
            from: None,
            to: None,
            source: None,
            unchecked: Vec::new(),
            reason: None,
            step: None,
            code: None,
        }
    }

    /// Read report bytes.
    ///
    /// # Errors
    /// Refuses oversized, malformed or unknown-schema bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, PackReportError> {
        if bytes.len() > MAX_PACK_REPORT_BYTES {
            return Err(PackReportError);
        }
        let report: Self = serde_json::from_slice(bytes).map_err(|_| PackReportError)?;
        (report.schema == PACK_REPORT_SCHEMA).then_some(report).ok_or(PackReportError)
    }

    /// The report's bytes.
    ///
    /// # Errors
    /// Returns an error only if serialization itself fails.
    pub fn render(&self) -> Result<Vec<u8>, PackReportError> {
        serde_json::to_vec(self).map_err(|_| PackReportError)
    }

    /// The one plain line a start shows about this report, if it needs one: an update, a
    /// newer version available, a failure to check or update, or the previous version
    /// running again.
    pub fn notice(&self, words: &PackUpdateMessages) -> Option<String> {
        let version = |text: &str| {
            self.to.as_deref().map(|version| UpdateMessages::with_version(text, version))
        };
        match self.outcome {
            PackOutcome::Updated => version(words.updated),
            PackOutcome::Available => version(words.available),
            PackOutcome::Restored => Some(words.previous_running.to_owned()),
            PackOutcome::CouldNotCheck => Some(words.could_not_check.to_owned()),
            PackOutcome::Failed => Some(words.could_not_update.to_owned()),
            PackOutcome::UpToDate
            | PackOutcome::Judging
            | PackOutcome::NotManaged
            | PackOutcome::Busy
            | PackOutcome::Stopped => None,
        }
    }

    /// The update report's plain lines.
    pub fn lines(&self, words: &PackUpdateMessages) -> Vec<String> {
        let mut lines = Vec::new();
        let first = match self.outcome {
            PackOutcome::Updated | PackOutcome::Available | PackOutcome::Restored => {
                self.notice(words)
            }
            PackOutcome::UpToDate | PackOutcome::Judging => Some(words.up_to_date.to_owned()),
            PackOutcome::NotManaged => Some(
                match self.reason.as_deref() {
                    Some("git-checkout") => words.kept_git,
                    Some("changed-files") => words.kept_edited,
                    Some("linked-folder" | "linked-content") => words.kept_linked,
                    _ => words.kept_unregistered,
                }
                .to_owned(),
            ),
            PackOutcome::CouldNotCheck | PackOutcome::Stopped => {
                Some(words.could_not_check.to_owned())
            }
            PackOutcome::Busy => Some(words.busy.to_owned()),
            PackOutcome::Failed => Some(words.could_not_update.to_owned()),
        };
        lines.extend(first);
        if self.outcome == PackOutcome::Failed {
            let step = match self.step.as_deref() {
                Some("downloading") => 1,
                Some("installing") => 2,
                _ => 0,
            };
            lines.push(words.steps[step].to_owned());
        }
        for source in &self.unchecked {
            lines.push(
                if source == "registry" {
                    words.registry_unchecked
                } else {
                    words.github_unchecked
                }
                .to_owned(),
            );
        }
        lines
    }
}

/// The live SugarCubes update state a coordinator publishes to its window.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum PackUpdateStatus {
    /// No SugarCubes update ran in this start.
    #[default]
    Idle,
    /// A SugarCubes update is running and holds the backend start.
    Updating,
    /// The start's SugarCubes update finished with this report.
    Finished(PackUpdateReport),
}

impl PackUpdateStatus {
    /// The splash line while a SugarCubes update holds the start.
    pub fn splash_line(&self, words: &PackUpdateMessages) -> Option<&'static str> {
        matches!(self, Self::Updating).then_some(words.updating)
    }

    /// The one plain line to show once the update finished, if any.
    pub fn notice(&self, words: &PackUpdateMessages) -> Option<String> {
        match self {
            Self::Finished(report) => report.notice(words),
            Self::Idle | Self::Updating => None,
        }
    }
}

#[cfg(test)]
#[path = "pack_tests.rs"]
mod tests;
