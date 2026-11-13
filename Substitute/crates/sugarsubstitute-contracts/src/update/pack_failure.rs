// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A SugarCubes update that failed during start-up, as the window's update report shows it.
//!
//! The host that keeps SugarCubes' own update report turns a failed one into this value: the
//! version it tried to install, the failed step and the report's plain lines, both already in
//! the user's language from the owner of SugarCubes update words, and the diagnostic codes worth
//! handing over. Every piece is redacted and bounded here, so whatever the host passes, the
//! window never shows or exports a path, an address or a credential.
use super::attempt::{MAX_CAUSES, MAX_VERSION_BYTES, sanitize};

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
    fn a_failure_is_bounded_however_much_the_host_passes() {
        let many = vec!["line"; 40];
        let failure = PackUpdateFailure::new(Some(&"1.".repeat(100)), Some(""), many.clone(), many);
        assert_eq!(failure.version().map(str::len), Some(MAX_VERSION_BYTES));
        assert_eq!(failure.step(), None);
        assert_eq!(failure.lines().len(), MAX_CAUSES);
        assert_eq!(failure.causes().len(), MAX_CAUSES);
    }
}
