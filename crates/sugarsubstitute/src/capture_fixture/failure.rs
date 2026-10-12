// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain first and bounded secondary causes without exposing paths or payloads.
use super::*;
use std::fmt;
/// Preserve one primary and eight secondary causes with redacted formatting.
pub(super) struct Failures {
    causes: [Option<Arc<dyn Error>>; 9],
    count: usize,
    omitted: u64,
}
impl Failures {
    /// Allocate the fixed cause slots before native startup.
    pub(super) fn new() -> Self {
        Self { causes: std::array::from_fn(|_| None), count: 0, omitted: 0 }
    }
    /// Report whether any original failure has been retained.
    pub(super) fn is_empty(&self) -> bool {
        self.count == 0
    }
    /// Report retained causes without formatting their contents.
    pub(super) fn count(&self) -> usize {
        self.count
    }
    /// Make excess secondary failures observable while preserving bounded storage.
    pub(super) fn omitted(&self) -> u64 {
        self.omitted
    }
    /// Retain the first cause permanently and count overflow categorically.
    pub(super) fn record(&mut self, cause: Box<dyn Error>) {
        if let Some(slot) = self.causes.get_mut(self.count) {
            *slot = Some(Arc::from(cause));
            self.count += 1;
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
}
impl fmt::Debug for Failures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureFixtureFailures")
            .field("retained", &self.count)
            .field("omitted", &self.omitted)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for Failures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Capture fixture has {} retained failures", self.count)
    }
}
impl Error for Failures {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.causes[0].as_deref()
    }
}
/// Preserve operation failure as the first source when receipt writing also fails.
pub(super) fn combine(primary: Result, secondary: Result) -> Result {
    let mut failures = Failures::new();
    if let Err(cause) = primary {
        failures.record(cause);
    }
    if let Err(cause) = secondary {
        failures.record(cause);
    }
    if failures.is_empty() { Ok(()) } else { Err(Box::new(failures)) }
}
