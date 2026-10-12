// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own entry completion policy through retained typed execution lifetimes.
use super::*;
impl CoordinatorEntryOwner {
    /// Consume at most32 exact notices and transfer one cached causal fact.
    /// # Errors
    /// Rejects excessive batches or original clock failure; native causes remain typed.
    pub fn consume(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<Option<EntryObservation>, EntryError> {
        if batch.notifications.len() > 32 {
            return Err(EntryError(ClaimError::invalid("entry_completion_bound")));
        }
        if let Some(pending) = &mut self.pending {
            let matched = batch.notifications.iter().any(|notice| {
                (notice.scope_id, notice.job_id) == (pending.job.scope_id(), pending.job.spec().id)
            });
            if matched && let Some(completion) = pending.job.try_take_completion() {
                let cleanup = pending.cleanup;
                self.pending = None;
                if let Err(cause) = self.completed(completion.outcome) {
                    self.close_admission();
                    return Err(cause);
                }
                if cleanup
                    && self.lifetime.retired.load(Ordering::Acquire)
                    && self.phase != EntryPhase::Settled
                {
                    self.phase = EntryPhase::Settled;
                    self.push(EntryObservation::Retired)?;
                }
            }
        }
        Ok(self.pop())
    }
    fn completed(&mut self, outcome: JobOutcome<Actual, ClaimError>) -> Result<(), EntryError> {
        let live = self.lifetime.open.load(Ordering::Acquire);
        match outcome {
            JobOutcome::Success(Actual::Closed) => {
                self.phase = EntryPhase::Settled;
                self.push(EntryObservation::Retired)?;
            }
            JobOutcome::Success(actual) if live => {
                let now = self
                    .client
                    .as_ref()
                    .ok_or_else(|| EntryError(ClaimError::invalid("entry_completion_execution")))?
                    .now()
                    .map_err(|cause| {
                        EntryError(ClaimError::new("entry_completion_clock", cause))
                    })?;
                if now >= self.budget.until() || now < self.budget.started() {
                    self.close_admission();
                    self.push(EntryObservation::Stopped(
                        sugarsubstitute_contracts::execution::TerminalKind::TimedOut,
                    ))?;
                    return Ok(());
                }
                match actual {
                    Actual::Discovered(known) => {
                        self.discovered = Some(known);
                        self.phase = EntryPhase::Discovered;
                        self.push(EntryObservation::NamespacePrepared(known.facts.namespace()))?;
                    }
                    Actual::Elected(Elected::Fresh { origin, inode }) => {
                        self.retain_election(ElectionRecordOrigin::Fresh(origin), inode)?;
                    }
                    Actual::Elected(Elected::OrdinaryReleased { record, inode }) => {
                        self.retain_election(ElectionRecordOrigin::Released(record), inode)?;
                    }
                    Actual::Elected(Elected::Candidate(epoch)) => {
                        self.candidate = Some(epoch);
                        self.phase = EntryPhase::Candidate;
                        self.push(EntryObservation::Candidate(epoch))?;
                    }
                    Actual::Elected(Elected::Contended) => {
                        self.phase = EntryPhase::Contended;
                        self.push(EntryObservation::Contended)?;
                    }
                    Actual::Elected(Elected::Degraded(reason)) => {
                        self.close_admission();
                        self.push(EntryObservation::Degraded(reason))?;
                    }
                    Actual::Closed => {
                        return Err(EntryError(ClaimError::invalid("entry_completion_action")));
                    }
                }
            }
            JobOutcome::Success(_) => {
                self.close_admission();
                self.push(EntryObservation::Stopped(
                    sugarsubstitute_contracts::execution::TerminalKind::Cancelled,
                ))?;
            }
            JobOutcome::Failure(cause) => {
                self.close_admission();
                self.push(EntryObservation::Failed(EntryError(cause)))?;
            }
            JobOutcome::Panicked(cause) => {
                self.close_admission();
                self.push(EntryObservation::Failed(EntryError(ClaimError::new(
                    "entry_worker_panic",
                    std::io::Error::other(cause.as_str().to_owned()),
                ))))?;
            }
            stopped => {
                let kind = stopped.kind();
                self.close_admission();
                self.push(EntryObservation::Stopped(kind))?;
            }
        }
        Ok(())
    }
    fn push(&mut self, event: EntryObservation) -> Result<(), EntryError> {
        let Some(slot) = self.events.iter_mut().find(|slot| slot.is_none()) else {
            self.close_admission();
            return Err(EntryError(ClaimError::invalid("entry_observation_capacity")));
        };
        *slot = Some(event);
        Ok(())
    }
    fn retain_election(
        &mut self,
        origin: ElectionRecordOrigin,
        inode: [u8; 24],
    ) -> Result<(), EntryError> {
        let known = self
            .discovered
            .ok_or_else(|| EntryError(ClaimError::invalid("entry_election_namespace")))?;
        let execution = self
            .execution
            .ok_or_else(|| EntryError(ClaimError::invalid("entry_election_execution")))?;
        self.election = Some(RetainedElectionProof {
            lifetime: self.lifetime.clone(),
            execution,
            namespace: known.facts.namespace(),
            installation_identity: known.installation_identity,
            account_sid: known.facts.account_sid().to_vec(),
            entry_inode: inode,
            budget: self.budget,
            origin,
            claim_lifetime: None,
        });
        self.phase = EntryPhase::ElectionHeld;
        self.push(EntryObservation::ElectionHeld)
    }
    fn pop(&mut self) -> Option<EntryObservation> {
        let event = self.events[0].take()?;
        for index in 1..4 {
            self.events[index - 1] = self.events[index].take();
        }
        Some(event)
    }
}
