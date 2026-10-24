// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Produce complete replacement records; rejected events never partially mutate selection.
use super::*;

impl InstallationRecord {
    /// Reserve one staging generation while retaining the accepted application.
    /// The adapter verifies package trust and obtains preparation ownership first.
    /// # Errors
    /// Rejects concurrent candidates, reused directory identities and quarantined automatic offers.
    pub fn prepare(
        &self,
        generation: Generation,
        intent: PreparationIntent,
    ) -> Result<Self, Rejection> {
        if self.candidate.is_some() {
            return Err(Rejection::Phase);
        }
        self.stage(generation, intent)
    }

    /// Replace one exact unlaunched candidate after the adapter owns preparation exclusion
    /// and authenticates the replacement. Old payloads remain retained; this grants no cleanup.
    /// # Errors
    /// Rejects trials, stale candidates, reused directories, quarantine and revision exhaustion.
    pub fn supersede_preparation(
        &self,
        expected: &Generation,
        replacement: Generation,
        intent: PreparationIntent,
    ) -> Result<Self, Rejection> {
        let pending = match &self.candidate {
            Some(Candidate::Staging { generation } | Candidate::Prepared { generation }) => {
                generation
            }
            _ => return Err(Rejection::Phase),
        };
        if pending != expected || replacement.id == pending.id {
            return Err(Rejection::Binding);
        }
        self.stage(replacement, intent)
    }

    /// Share failed-package admission and complete-record validation across fresh reservations.
    fn stage(&self, generation: Generation, intent: PreparationIntent) -> Result<Self, Rejection> {
        if intent == PreparationIntent::Automatic && self.rejected.contains(&generation.package) {
            return Err(Rejection::Quarantined);
        }
        let mut next = self.advance()?;
        next.candidate = Some(Candidate::Staging { generation });
        next.validate()?;
        Ok(next)
    }
    /// Mark the exact staging generation complete after native package verification.
    /// # Errors
    /// Rejects wrong-generation, duplicate and out-of-phase completions.
    pub fn prepared(&self, generation: &Generation) -> Result<Self, Rejection> {
        let Some(Candidate::Staging { generation: pending }) = &self.candidate else {
            return Err(Rejection::Phase);
        };
        if pending != generation {
            return Err(Rejection::Binding);
        }
        let mut next = self.advance()?;
        next.candidate = Some(Candidate::Prepared { generation: generation.clone() });
        Ok(next)
    }
    /// Admit a candidate trial after the adapter establishes exclusive launch/activation custody.
    /// Persist this record before starting the candidate; the new trial binds its health receipt.
    /// # Errors
    /// Rejects incomplete preparation and revision exhaustion.
    pub fn begin_trial(&self) -> Result<Self, Rejection> {
        let Some(Candidate::Prepared { generation }) = &self.candidate else {
            return Err(Rejection::Phase);
        };
        let mut next = self.advance()?;
        next.candidate = Some(Candidate::Trial {
            attempt: Trial {
                installation: self.installation.clone(),
                generation: generation.clone(),
                revision: next.revision,
            },
        });
        Ok(next)
    }
    /// Accept only the exact attempt whose health was verified through authenticated native IPC.
    /// This changes selection only; compatibility-aware cleanup is a separate storage operation.
    /// # Errors
    /// Rejects stale, duplicate, cross-installation and wrong-generation acknowledgements.
    pub fn accept(&self, attempt: &Trial) -> Result<Self, Rejection> {
        self.match_trial(attempt)?;
        let mut next = self.advance()?;
        next.retained = next.accepted.replace(attempt.generation.clone());
        next.candidate = None;
        next.rejected.retain(|digest| digest != &attempt.generation.package);
        next.validate()?;
        Ok(next)
    }
    /// Reject an exact failed trial only after native custody confirms it has settled.
    /// Merely loading a trial record never establishes that its owner crashed or exited.
    /// # Errors
    /// Rejects stale or unbound failure reports and revision exhaustion.
    pub fn reject(&self, attempt: &Trial) -> Result<Self, Rejection> {
        self.match_trial(attempt)?;
        let mut next = self.advance()?;
        next.candidate = None;
        next.rejected.retain(|digest| digest != &attempt.generation.package);
        next.rejected.push(attempt.generation.package.clone());
        if next.rejected.len() > MAX_REJECTED_PACKAGES {
            next.rejected.remove(0);
        }
        next.validate()?;
        Ok(next)
    }
    /// Restore preparation when the native creation owner proves no process was created.
    /// An absent lock, elapsed time or a loaded journal is not that proof. The adapter must
    /// permanently close this attempt's creation admission before publishing this proposal.
    /// # Errors
    /// Rejects a stale, foreign or already completed attempt and revision exhaustion.
    pub fn cancel_unstarted_trial(&self, attempt: &Trial) -> Result<Self, Rejection> {
        self.match_trial(attempt)?;
        let mut next = self.advance()?;
        next.candidate = Some(Candidate::Prepared { generation: attempt.generation.clone() });
        next.validate()?;
        Ok(next)
    }
    /// Abandon preparation without marking a user's cancelled download as a broken release.
    /// The adapter must stop and observe the preparation worker before using this proposal.
    /// # Errors
    /// Rejects an active trial, absent candidate or another generation's cancellation.
    pub fn cancel_preparation(&self, generation: &Generation) -> Result<Self, Rejection> {
        let pending = match &self.candidate {
            Some(Candidate::Staging { generation }) | Some(Candidate::Prepared { generation }) => {
                generation
            }
            _ => return Err(Rejection::Phase),
        };
        if pending != generation {
            return Err(Rejection::Binding);
        }
        let mut next = self.advance()?;
        next.candidate = None;
        Ok(next)
    }
    fn match_trial(&self, attempt: &Trial) -> Result<(), Rejection> {
        match &self.candidate {
            Some(Candidate::Trial { attempt: pending }) if pending == attempt => Ok(()),
            Some(Candidate::Trial { .. }) => Err(Rejection::Binding),
            _ => Err(Rejection::Phase),
        }
    }
    fn advance(&self) -> Result<Self, Rejection> {
        let mut next = self.clone();
        next.revision = next.revision.checked_add(1).ok_or(Rejection::Capacity)?;
        Ok(next)
    }
}
