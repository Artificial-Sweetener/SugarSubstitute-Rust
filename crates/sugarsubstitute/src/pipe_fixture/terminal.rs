// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one exact terminal notification across fallible observation handling.
use super::Result;
use sugarsubstitute_contracts::execution::{CompletionBatch, JobId, ScopeId, TerminalKind};

#[derive(Default)]
pub(super) struct PendingTerminal(Option<(ScopeId, JobId, TerminalKind)>);
impl PendingTerminal {
    /// Capture metadata before processing observations, which may fail independently.
    pub(super) fn capture(
        &mut self,
        scope: ScopeId,
        job: JobId,
        batch: &CompletionBatch,
    ) -> Result<()> {
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) == (scope, job) {
                let actual = (scope, job, notice.kind);
                if self.0.is_some_and(|retained| retained != actual) {
                    return Err(
                        "Lifecycle terminal notification changed retained identity/kind".into()
                    );
                }
                self.0 = Some(actual);
            }
        }
        Ok(())
    }
    /// Metadata permits typed transfer; it never represents native settlement itself.
    pub(super) fn kind(&self) -> Option<TerminalKind> {
        self.0.map(|(_, _, kind)| kind)
    }
}
