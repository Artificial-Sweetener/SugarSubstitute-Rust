// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Produce complete replacement records; rejected events never partially mutate selection.
use super::*;

impl InstallationRecord {
    /// Reserve one staging generation while retaining the accepted application.
    /// The adapter verifies package trust and obtains preparation ownership first.
    /// # Errors
    /// Rejects concurrent candidates, reused directory identities and automatic offers withheld at
    /// `now` (UTC Unix seconds).
    pub fn prepare(
        &self,
        generation: Generation,
        intent: PreparationIntent,
        now: u64,
    ) -> Result<Self, Rejection> {
        if self.candidate.is_some() {
            return Err(Rejection::Phase);
        }
        self.stage(generation, intent, now)
    }

    /// Replace one exact unlaunched candidate after the adapter owns preparation exclusion
    /// and authenticates the replacement. Old payloads remain retained; this grants no cleanup.
    /// # Errors
    /// Rejects trials, stale candidates, reused directories, packages withheld at `now` (UTC Unix
    /// seconds) and revision exhaustion.
    pub fn supersede_preparation(
        &self,
        expected: &Generation,
        replacement: Generation,
        intent: PreparationIntent,
        now: u64,
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
        self.stage(replacement, intent, now)
    }

    /// Share failed-package admission and complete-record validation across fresh reservations.
    fn stage(
        &self,
        generation: Generation,
        intent: PreparationIntent,
        now: u64,
    ) -> Result<Self, Rejection> {
        if intent == PreparationIntent::Automatic && self.withheld(&generation.package, now) {
            return Err(Rejection::Quarantined);
        }
        let mut next = self.advance()?;
        if next
            .unwitnessed
            .as_ref()
            .is_some_and(|abandoned| abandoned.package != generation.package)
        {
            next.unwitnessed = None;
        }
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
        next.strikes.retain(|strike| strike.package != attempt.generation.package);
        next.unwitnessed = None;
        next.rollback = None;
        next.repair = None;
        next.validate()?;
        Ok(next)
    }
    /// Record a bad verdict for an exact settled trial: the new host itself crashed or failed its
    /// own startup. The accepted generation keeps running, the rejected generation becomes
    /// `rollback`, and the package gains one strike dated `at` (UTC Unix seconds).
    /// Merely loading a trial record never establishes that its owner crashed or exited.
    /// # Errors
    /// Rejects stale or unbound failure reports and revision exhaustion.
    pub fn reject(&self, attempt: &Trial, at: u64) -> Result<Self, Rejection> {
        self.match_trial(attempt)?;
        let mut next = self.advance()?;
        next.candidate = None;
        next.unwitnessed = None;
        next.strike(&attempt.generation, at);
        next.rollback = Some(attempt.generation.clone());
        next.validate()?;
        Ok(next)
    }
    /// End an exact trial whose process existed without a verdict, returning its candidate to
    /// `Prepared` so the next launch tries it again. Use it for every observed ending that is not
    /// the new host's own failure: a close request, a backend failure, a session end, a forced end
    /// or an unpublished acceptance. Strikes are unchanged; an abandoned-trial streak ends because
    /// someone observed this trial's end.
    /// # Errors
    /// Rejects a stale, foreign or already completed attempt and revision exhaustion.
    pub fn release_trial(&self, attempt: &Trial) -> Result<Self, Rejection> {
        self.match_trial(attempt)?;
        let mut next = self.advance()?;
        next.candidate = Some(Candidate::Prepared { generation: attempt.generation.clone() });
        next.unwitnessed = None;
        next.validate()?;
        Ok(next)
    }
    /// Resolve an exact trial found abandoned at a later launch, whose ending nobody observed: a
    /// power cut, a kill of every owner, or a host crash before its custody witness enrolled.
    /// The candidate returns to `Prepared` without a strike, unless this makes
    /// [`UNWITNESSED_LIMIT`] abandoned trials of the package in a row; that one counts as a bad
    /// verdict dated `at` (UTC Unix seconds), so a host that dies unobserved is not retried at
    /// every launch forever.
    /// # Errors
    /// Rejects a stale, foreign or already completed attempt and revision exhaustion.
    pub fn abandon_trial(&self, attempt: &Trial, at: u64) -> Result<Self, Rejection> {
        self.match_trial(attempt)?;
        let package = &attempt.generation.package;
        let count = match &self.unwitnessed {
            Some(abandoned) if &abandoned.package == package => abandoned.count.saturating_add(1),
            _ => 1,
        };
        if count >= UNWITNESSED_LIMIT {
            return self.reject(attempt, at);
        }
        let mut next = self.advance()?;
        next.candidate = Some(Candidate::Prepared { generation: attempt.generation.clone() });
        next.unwitnessed = Some(Unwitnessed { package: package.clone(), count });
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
    /// Run the retained generation again because the accepted `failed` generation's files are
    /// missing or damaged (for example quarantined by antivirus), so its engine could not start.
    /// The retained generation becomes accepted and `failed` is recorded for repair: preparation
    /// downloads and verifies the same release again, and accepting it clears the repair. This is
    /// not a bad verdict, so it adds no strike and no rollback. A staging or prepared candidate is
    /// dropped because the failed generation's engine prepared it.
    /// # Errors
    /// `Binding` unless `failed` is the accepted generation; `Phase` without a retained
    /// generation or during a trial; `Capacity` on revision exhaustion.
    pub fn fall_back_to_retained(&self, failed: &Generation) -> Result<Self, Rejection> {
        if self.accepted.as_ref() != Some(failed) {
            return Err(Rejection::Binding);
        }
        if self.retained.is_none() || matches!(self.candidate, Some(Candidate::Trial { .. })) {
            return Err(Rejection::Phase);
        }
        let mut next = self.advance()?;
        next.accepted = next.retained.take();
        next.repair = Some(failed.clone());
        next.candidate = None;
        next.unwitnessed = None;
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
    /// Count one bad verdict; a strike past its lifetime restarts at one. The struck package
    /// becomes the most recent entry, evicting the oldest beyond capacity.
    fn strike(&mut self, generation: &Generation, at: u64) {
        let package = &generation.package;
        let count = match self.strikes.iter().position(|strike| &strike.package == package) {
            Some(index) => {
                let previous = self.strikes.remove(index);
                if previous.live(at) { previous.count.saturating_add(1) } else { 1 }
            }
            None => 1,
        };
        self.strikes.push(Strike {
            package: package.clone(),
            sequence: generation.release.sequence(),
            count,
            at,
        });
        if self.strikes.len() > MAX_STRUCK_PACKAGES {
            self.strikes.remove(0);
        }
    }
    fn advance(&self) -> Result<Self, Rejection> {
        let mut next = self.clone();
        next.revision = next.revision.checked_add(1).ok_or(Rejection::Capacity)?;
        Ok(next)
    }
}
