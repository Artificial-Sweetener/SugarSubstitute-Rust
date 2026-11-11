// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! What one update attempt is doing and how it ended, for the splash, status and update report.
//!
//! These values describe an attempt; whether an update is prepared remains the selection
//! record's state. Text in a report is diagnostic content the user may hand over, never product
//! copy: product words come from the localization owner.
use serde::{Deserialize, Serialize};
pub mod codec;

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

    /// Plain text for Copy and Export: version, phase, then one cause per line.
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "Update to {}\nPhase: {}\n",
            self.version.as_deref().unwrap_or("unknown version"),
            serde_json::to_value(self.phase)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default()
        );
        for cause in &self.causes {
            text.push_str("Cause: ");
            text.push_str(cause);
            text.push('\n');
        }
        text
    }
}

/// Replace local paths, network addresses and credential-looking tokens, collapse whitespace,
/// and bound the result so a report can be handed over as it is.
pub fn sanitize(text: &str) -> String {
    let mut words = Vec::new();
    for word in text.split_whitespace() {
        let bare = word.trim_matches(|c: char| matches!(c, '"' | '\'' | '(' | ')' | ',' | ';'));
        words.push(if bare.contains("://") {
            "<address>"
        } else if is_path(bare) {
            "<path>"
        } else if is_secret(bare) {
            "<redacted>"
        } else {
            word
        });
    }
    bounded(&words.join(" "), MAX_CAUSE_BYTES)
}

/// Drive-qualified, UNC, verbatim and rooted POSIX paths, including ones inside a token.
fn is_path(word: &str) -> bool {
    let bytes = word.as_bytes();
    word.starts_with("\\\\")
        || (word.starts_with('/') && word.len() > 1)
        || bytes.windows(3).any(|window| {
            window[0].is_ascii_alphabetic()
                && window[1] == b':'
                && matches!(window[2], b'\\' | b'/')
        })
}

/// Key-value credentials and bearer-style tokens.
fn is_secret(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    ["token=", "password=", "secret=", "key=", "ticket=", "signature=", "authorization:"]
        .iter()
        .any(|marker| lower.contains(marker))
}

/// Truncate at a character boundary within `maximum` bytes.
fn bounded(text: &str, maximum: usize) -> String {
    if text.len() <= maximum {
        return text.to_owned();
    }
    let mut end = maximum;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
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
        assert_eq!(
            report.to_text(),
            "Update to 2.0.0\nPhase: downloading\nCause: Update repository download failed\nCause: Update package I/O failed\n"
        );
    }

    #[test]
    fn paths_addresses_and_credentials_never_reach_a_report() {
        let text = sanitize(
            r#"Cannot open "C:\Users\someone\SugarSubstitute\user\x.json" or \\server\share\f via https://updates.example.test/a?ticket=1 token=abc /home/someone/x"#,
        );
        assert_eq!(text, "Cannot open <path> or <path> via <address> <redacted> <path>");
        assert!(!text.contains("someone"));
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
