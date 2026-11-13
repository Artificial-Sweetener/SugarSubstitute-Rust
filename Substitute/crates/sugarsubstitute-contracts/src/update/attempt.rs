// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! What one update attempt is doing and how it ended, for the splash, status and update report.
//!
//! These values describe an attempt; whether an update is prepared remains the selection
//! record's state. Text in a report is diagnostic content the user may hand over, never product
//! copy: product words come from the localization owner.
use serde::{Deserialize, Serialize};
pub mod codec;
mod redact;
use redact::bounded;
pub use redact::{ADDRESS, PATH, REDACTED, sanitize};

/// Most causes retained in one report.
pub const MAX_CAUSES: usize = 12;
/// Longest single cause, in bytes, after sanitizing.
pub const MAX_CAUSE_BYTES: usize = 400;
/// Longest version label, in bytes.
pub const MAX_VERSION_BYTES: usize = 64;

/// Stage of an attempt, as reported while it runs and recorded when it fails.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptPhase {
    /// Opening local update trust and the subscription.
    Opening,
    /// Fetching and authenticating repository metadata.
    Checking,
    /// Selecting or checking the update engine needed for this release.
    CheckingUpdater,
    /// Transferring the package; byte counts accompany progress.
    Downloading,
    /// Verifying, extracting and staging the prepared version.
    Preparing,
    /// Staging and probing a newer update engine.
    PreparingUpdater,
    /// Starting the prepared version.
    Applying,
}

/// Live progress of a running attempt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptProgress {
    /// Current stage.
    pub phase: AttemptPhase,
    /// Durable package bytes and authenticated total while downloading.
    pub bytes: Option<(u64, u64)>,
}

/// Terminal result of one attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttemptOutcome {
    /// The installed version is current.
    Current,
    /// A newer version is prepared and activates at the next launch.
    Prepared {
        /// Version label of the prepared release.
        version: String,
    },
    /// The release needs the installer; automatic updating cannot apply it.
    NeedsInstaller,
    /// Another preparation owner is already working; this attempt did nothing.
    AlreadyPreparing,
    /// The attempt failed; the report says where and why.
    Failed(UpdateReport),
}

/// A readable account of one failed attempt: version, phase and cause chain, without secrets,
/// addresses or local paths.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateReport {
    /// Version the attempt was updating to, when the offer had been authenticated.
    pub version: Option<String>,
    /// Stage at which the attempt failed.
    pub phase: AttemptPhase,
    /// Outermost cause first, sanitized and bounded.
    pub causes: Vec<String>,
}
impl UpdateReport {
    /// Build from an original error chain. Each cause is sanitized and bounded, consecutive
    /// repeats are dropped (wrappers often repeat their source), and at most `MAX_CAUSES` remain.
    pub fn from_error(
        version: Option<&str>,
        phase: AttemptPhase,
        error: &(dyn std::error::Error + 'static),
    ) -> Self {
        let mut causes: Vec<String> = Vec::new();
        let mut current = Some(error);
        while let Some(cause) = current {
            if causes.len() == MAX_CAUSES {
                break;
            }
            let text = sanitize(&cause.to_string());
            if !text.is_empty() && causes.last() != Some(&text) {
                causes.push(text);
            }
            current = cause.source();
        }
        Self {
            version: version.map(|value| bounded(value.trim(), MAX_VERSION_BYTES)),
            phase,
            causes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{error::Error, fmt};

    #[derive(Debug)]
    struct Layer(&'static str, Option<Box<Layer>>);
    impl fmt::Display for Layer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }
    impl Error for Layer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.1.as_deref().map(|layer| layer as &(dyn Error + 'static))
        }
    }

    #[test]
    fn report_keeps_the_cause_chain_outermost_first_without_repeats() {
        let error = Layer(
            "Update repository download failed",
            Some(Box::new(Layer(
                "Update package I/O failed",
                Some(Box::new(Layer("Update package I/O failed", None))),
            ))),
        );
        let report = UpdateReport::from_error(Some("2.0.0"), AttemptPhase::Downloading, &error);
        assert_eq!(report.version.as_deref(), Some("2.0.0"));
        assert_eq!(
            report.causes,
            ["Update repository download failed", "Update package I/O failed"]
        );
    }

    #[test]
    fn every_cause_of_a_chain_is_redacted_before_it_reaches_the_report() {
        let error = Layer(
            r"Update package could not be staged in C:\Users\Jo Ann\SugarSubstitute\releases",
            Some(Box::new(Layer(
                "proxy 10.0.0.7:3128 refused Authorization: Bearer abc.def",
                None,
            ))),
        );
        let report = UpdateReport::from_error(None, AttemptPhase::Preparing, &error);
        assert_eq!(
            report.causes,
            ["Update package could not be staged in <path>", "proxy <address> refused <redacted>"]
        );
    }

    #[test]
    fn causes_and_versions_are_bounded_at_character_boundaries() {
        let long = "é".repeat(MAX_CAUSE_BYTES);
        let cause = sanitize(&long);
        assert!(cause.len() <= MAX_CAUSE_BYTES);
        assert!(cause.chars().all(|c| c == 'é'));
        let chain = (0..20).fold(None, |inner: Option<Box<Layer>>, index| {
            Some(Box::new(Layer(["a", "b"][index % 2], inner)))
        });
        let error = *chain.unwrap_or_else(|| Box::new(Layer("unreachable", None)));
        let report =
            UpdateReport::from_error(Some(&"9".repeat(200)), AttemptPhase::Checking, &error);
        assert_eq!(report.causes.len(), MAX_CAUSES);
        assert_eq!(report.version.map(|version| version.len()), Some(MAX_VERSION_BYTES));
    }

    #[test]
    fn report_wire_form_rejects_unknown_fields() {
        let decoded = serde_json::from_str::<UpdateReport>(
            r#"{"version":null,"phase":"checking","causes":[],"path":"x"}"#,
        );
        assert!(decoded.is_err());
    }
}
