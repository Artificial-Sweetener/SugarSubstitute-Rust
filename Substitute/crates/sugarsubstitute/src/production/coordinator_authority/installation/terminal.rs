// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire completed publication scratch space while preserving owned failure diagnostics.
use super::*;
use std::io;

/// Unit-valued storage work returns no payload requiring its successful scratch reservation.
/// Failure and panic diagnostics retain their original completion and memory accounting.
pub(super) struct Terminal {
    kind: TerminalKind,
    failure: Option<JobCompletion<(), io::Error>>,
}
impl Terminal {
    /// Call after checking the actual completion against its original job and notification.
    pub(super) fn observe(completion: JobCompletion<(), io::Error>) -> Self {
        let kind = completion.outcome.kind();
        let failure =
            matches!(kind, TerminalKind::Failure | TerminalKind::Panicked).then_some(completion);
        Self { kind, failure }
    }

    /// Preserve the actual terminal even when its notification did not authenticate success.
    pub(super) fn kind(&self) -> TerminalKind {
        self.kind
    }

    /// Borrow the original storage error without discarding its retained reservation.
    pub(super) fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.failure.as_ref()?.outcome {
            JobOutcome::Failure(cause) => Some(cause),
            _ => None,
        }
    }
}
