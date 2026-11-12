// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain native first/secondary observations across consumed nested and outer replies.

use super::native_reply::{NativeCause, ReplyFacts};
use std::{error::Error, fmt};

/// Fixed original ledger lives in the actual output owner before any callback.
/// Re-observing the same primary does not invent another secondary occurrence.
#[derive(Clone, Copy, Default)]
pub(super) struct NativeFailureLedger {
    primary: Option<NativeCause>,
    secondary: [Option<NativeCause>; 8],
    count: usize,
    omitted: u64,
}

impl NativeFailureLedger {
    /// Adopt actual newly returned secondary observations before the next native call.
    pub(super) fn adopt(&mut self, facts: &ReplyFacts) {
        if let Some(primary) = facts.primary {
            match self.primary {
                None => self.primary = Some(primary),
                Some(original) if original == primary => {}
                Some(_) => self.append(primary),
            }
        }
        for cause in facts.secondary.iter().flatten() {
            self.append(*cause);
        }
        self.omitted = self.omitted.saturating_add(facts.omitted);
    }

    /// Keep the original bounded facts without allocating or consulting native state.
    pub(super) fn failure(&self) -> Option<NativeFailure> {
        self.primary.map(|primary| NativeFailure {
            primary,
            secondary: self.secondary,
            omitted: self.omitted,
        })
    }

    /// Bound new real observations; saturated omission counts never wrap.
    fn append(&mut self, cause: NativeCause) {
        if self.count < self.secondary.len() {
            self.secondary[self.count] = Some(cause);
            self.count += 1;
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
}

/// One preserved producer-domain first cause with exact bounded later causes.
#[derive(Clone, Copy, Debug)]
pub(super) struct NativeFailure {
    primary: NativeCause,
    secondary: [Option<NativeCause>; 8],
    omitted: u64,
}

impl fmt::Display for NativeFailure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.primary.fmt(output)?;
        for secondary in self.secondary.iter().flatten() {
            write!(output, "; cleanup: {secondary}")?;
        }
        if self.omitted != 0 {
            write!(output, "; {} further native cleanup failures omitted", self.omitted)?;
        }
        Ok(())
    }
}

impl Error for NativeFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.primary)
    }
}

#[cfg(test)]
#[path = "native_failure_tests.rs"]
mod tests;
