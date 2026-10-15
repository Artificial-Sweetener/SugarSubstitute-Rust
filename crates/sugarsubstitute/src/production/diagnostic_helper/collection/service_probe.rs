// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive real policy-only service jobs while keeping capture and native cleanup unavailable.
use super::*;
impl CollectionService {
    /// Submit a pure policy-custody probe on the real service path, without native input or effects.
    pub(in crate::production) fn submit_policy_probe(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        specification: JobSpec,
        policy: &mut Option<FundedState>,
        boundary: impl FnOnce() + Send + 'static,
    ) -> Result<(), CollectionIssue> {
        if self.job.is_some() || self.completion.is_some() || self.installed {
            return Err(CollectionIssue::Busy);
        }
        let probe = self.source.policy_probe(policy)?;
        match client.submit_blocking(scope, specification, move |_context| {
            boundary();
            drop(probe);
            Ok(())
        }) {
            Ok(job) => {
                self.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Err(CollectionIssue::Admission)
            }
        }
    }
    /// Park real terminal adoption at the same policy-only boundary used by the production service.
    pub(in crate::production) fn capture_policy_probe_terminal(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<Option<JobSpec>, CollectionIssue> {
        let (_, adopted) = self.capture_terminal(batch)?;
        if adopted { Ok(self.completion.as_ref().map(|done| done.spec)) } else { Ok(None) }
    }
    /// Finish the exact pure probe's adopted return; this supplies no source/file-retirement proof.
    pub(in crate::production) fn restore_policy_probe(
        &mut self,
        policy: &mut Option<FundedState>,
    ) -> Result<(), CollectionIssue> {
        if self.job.is_some() || self.completion.is_none() || !self.source.policy_pending() {
            return Err(CollectionIssue::Busy);
        }
        self.restore_policy(policy)
    }
}
