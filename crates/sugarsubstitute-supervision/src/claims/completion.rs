// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish facts only after the exact finite job has actually returned.
use super::*;
use sugarsubstitute_contracts::execution::{JobOutcome, TerminalKind};
impl WindowsResourceClaims {
    /// Consume at most32 notifications through owner-retained typed jobs.
    /// Draining metadata does not acknowledge jobs or prove that native work stopped.
    pub fn consume(&mut self, batch: &CompletionBatch) -> Result<usize, ClaimError> {
        if batch.notifications.len() > 32 {
            return Err(ClaimError::invalid("claim_completion_bound"));
        }
        let mut matched = 0;
        for notice in &batch.notifications {
            let identity = (notice.scope_id, notice.job_id);
            let Some(index) = self.pending.iter().position(|entry| {
                entry
                    .as_ref()
                    .is_some_and(|entry| (entry.job.scope_id(), entry.job.spec().id) == identity)
            }) else {
                continue;
            };
            let completion =
                self.pending[index].as_mut().and_then(|entry| entry.job.try_take_completion());
            let Some(completion) = completion else {
                continue;
            };
            let Some(pending) = self.pending[index].take() else {
                return Err(ClaimError::invalid("claim_pending_identity"));
            };
            let kind = completion.outcome.kind();
            let mut failure = self.actual(pending.action);
            if failure.is_none() {
                failure = match completion.outcome {
                    JobOutcome::Failure(cause) => Some(cause),
                    JobOutcome::Panicked(cause) => Some(ClaimError::new(
                        "claim_worker_panic",
                        std::io::Error::other(cause.as_str().to_owned()),
                    )),
                    _ => None,
                };
            }
            if let Some(cause) = failure {
                self.close_admission();
                self.events
                    .push(ClaimObservation::Failed { target: pending.target.clone(), cause });
            }
            if matches!(
                kind,
                TerminalKind::Cancelled
                    | TerminalKind::TimedOut
                    | TerminalKind::Panicked
                    | TerminalKind::Coalesced
            ) {
                self.close_admission();
                self.events.push(ClaimObservation::Stopped { target: pending.target, kind });
            }
            matched += 1;
        }
        Ok(matched)
    }
    fn actual(&mut self, action: Action) -> Option<ClaimError> {
        match action {
            Action::Initialize => {
                self.initialization_pending = false;
                let root = lock(&self.root.value);
                if root.claimed {
                    self.owner = root.key;
                    self.initialized = true;
                    if let Some(owner) = root.key {
                        self.events.push(ClaimObservation::CoordinatorClaimed(owner));
                    }
                }
                lock(&self.root.failure).take()
            }
            Action::Resolve(index) => {
                let Some(entry) = self.resolutions[index].as_mut() else {
                    return Some(ClaimError::invalid("resolution_completion_identity"));
                };
                entry.pending = false;
                let proof = lock(&entry.shared.value).take().map(Arc::new);
                entry.failed = proof.as_ref().is_none_or(|proof| !proof.ready);
                if !entry.failed {
                    self.events.push(ClaimObservation::LaunchResolved(entry.ticket.clone()));
                }
                entry.proof = proof;
                lock(&entry.shared.failure).take()
            }
            Action::Claim(index) => {
                let Some(entry) = self.backends[index].as_mut() else {
                    return Some(ClaimError::invalid("backend_completion_identity"));
                };
                entry.pending = false;
                entry.claimed = lock(&entry.shared.value).claimed;
                let failure = lock(&entry.shared.failure).take();
                entry.failed = failure.is_some() || !entry.claimed;
                if entry.claimed {
                    self.events.push(ClaimObservation::BackendClaimed(entry.resource));
                }
                failure
            }
            Action::Retire(index) => {
                let Some(entry) = self.resolutions[index].as_mut() else {
                    return Some(ClaimError::invalid("retire_completion_identity"));
                };
                entry.pending = false;
                if entry.proof.as_ref().is_some_and(|proof| lock(&proof.handles).is_empty()) {
                    self.events.push(ClaimObservation::ResolutionRetired(entry.ticket.clone()));
                    self.resolutions[index] = None;
                }
                None
            }
            Action::Release(index) => {
                let Some(entry) = self.backends[index].as_mut() else {
                    return Some(ClaimError::invalid("release_completion_identity"));
                };
                entry.pending = false;
                let released = lock(&entry.shared.value).released;
                let failure = lock(&entry.shared.failure).take();
                if released {
                    self.events.push(ClaimObservation::BackendReleased(entry.resource));
                    self.backends[index] = None;
                }
                failure
            }
            Action::Close => {
                let root = lock(&self.root.value);
                let empty =
                    root.pool.is_none() && root.handles.is_empty() && root.coordinator.is_none();
                if empty {
                    self.closed = true;
                    self.events.push(ClaimObservation::NativeClaimsClosed);
                }
                lock(&self.root.failure).take()
            }
        }
    }
}
