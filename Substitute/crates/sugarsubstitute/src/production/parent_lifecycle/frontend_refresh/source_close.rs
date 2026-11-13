// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain uncertain source closure while returning matched successful job credit.
use super::*;
use sugarsubstitute_contracts::execution::{JobCompletion, Scope};
use sugarsubstitute_execution::ExecutionClient;

#[derive(Default)]
pub(super) struct SourceClose {
    job: Option<OwnedJob<(), FrontendSourceError>>,
    terminal: Option<JobCompletion<(), FrontendSourceError>>,
    closed: bool,
    foreign: bool,
}

impl SourceClose {
    pub(super) fn closed(&self) -> bool {
        self.closed
    }

    /// A completed failure retains custody but does not starve independent HTTP cleanup.
    pub(super) fn awaiting_source(&self) -> bool {
        !self.closed && self.terminal.is_none()
    }

    pub(super) fn needs_submission(&self) -> bool {
        self.awaiting_source() && self.job.is_none()
    }

    pub(super) fn submit(
        &mut self,
        source: &InstalledFrontendOwner,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<()> {
        if self.needs_submission() {
            self.job = Some(source.submit_close(client, scope, spec)?);
        }
        Ok(())
    }

    /// Only the original matching successful terminal permits credit release.
    pub(super) fn consume(&mut self) -> Result<()> {
        if let Some(job) = &mut self.job
            && let Some(terminal) = job.try_take_completion()
        {
            self.accept_terminal(terminal)?;
        }
        if self.foreign {
            return Err(ParentError::ForeignPeer.into());
        }
        if let Some(terminal) = &self.terminal {
            return match &terminal.outcome {
                JobOutcome::Failure(cause) => Err(cause.clone().into()),
                _ => Err(ParentError::OwnerFailed.into()),
            };
        }
        Ok(())
    }

    fn accept_terminal(&mut self, terminal: JobCompletion<(), FrontendSourceError>) -> Result<()> {
        self.foreign = self
            .job
            .as_ref()
            .is_none_or(|job| terminal.scope_id != job.scope_id() || terminal.spec != job.spec());
        self.terminal = Some(terminal);
        self.job = None;
        if self.foreign {
            return Err(ParentError::ForeignPeer.into());
        }
        if matches!(
            self.terminal.as_ref().map(|value| &value.outcome),
            Some(JobOutcome::Success(()))
        ) {
            self.closed = true;
            self.terminal = None;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "source_close/tests.rs"]
mod tests;
