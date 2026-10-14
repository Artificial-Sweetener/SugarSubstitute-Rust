// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve one original UI challenge and count only actual native issue and response.
use super::types::WritePurpose;
use super::{ParentError, ParentLifecycle, ParentResources, Result};
use sugarsubstitute_contracts::execution::Moment;
use sugarsubstitute_contracts::lifecycle::{
    UI_SUSPICION_THRESHOLD, UiChallengeId, UiChallengeKey, UiWatchdogActivity, UiWatchdogEffect,
    UiWatchdogObservation,
};
use sugarsubstitute_supervision::lifecycle_protocol::{WireHeader, WirePayload};

/// Original wire correlation and actual native issue evidence, funded by actor metadata.
#[derive(Clone, Copy)]
pub(super) struct Challenge {
    pub(super) key: UiChallengeKey,
    header: WireHeader,
    queued_at: Moment,
    until: Moment,
    operation: Option<u64>,
    issued_at: Option<Moment>,
}

impl ParentLifecycle {
    pub(super) fn send_challenge(&mut self, resources: &ParentResources<'_, '_>) -> Result<()> {
        if !self.registered || self.cleanup.is_some() {
            return Ok(());
        }
        let now = resources.execution.client.now()?;
        let snapshot = self.watchdog.as_ref().ok_or(ParentError::WrongPhase)?.snapshot();
        self.expire_retired_challenges(now);
        if self.challenge.is_some_and(|challenge| snapshot.pending != Some(challenge.key)) {
            self.retire_challenge(now)?;
        }
        let watchdog = self.watchdog.as_mut().ok_or(ParentError::WrongPhase)?;
        let effect = watchdog.poll(now)?;
        if self.challenge.is_none()
            && let Some(UiWatchdogEffect::Challenge { challenge }) = effect
        {
            let until = now.checked_add(UI_SUSPICION_THRESHOLD)?;
            let counter = resources.execution.client.encode_counter_deadline(until)?;
            self.challenge = Some(Challenge {
                key: challenge,
                header: WireHeader {
                    run: self.run,
                    connection: Some(self.connection),
                    request: None,
                    deadline_qpc: counter.ticks(),
                },
                queued_at: now,
                until,
                operation: None,
                issued_at: None,
            });
        }
        let Some(challenge) = self.challenge else {
            return Ok(());
        };
        if watchdog.snapshot().phase
            != Some(sugarsubstitute_application::supervision::UiChallengePhase::Queued)
        {
            return Ok(());
        }
        watchdog.validate_dispatch(self.run, challenge.key)?;
        if self.write_message(
            resources,
            challenge.header,
            WirePayload::UiChallenge(challenge.key.id),
            challenge.until,
            WritePurpose::Challenge(challenge.key),
        )? {
            let operation = self.write.as_ref().ok_or(ParentError::WrongPhase)?.receipt.operation;
            self.queue_challenge_write(challenge.key, operation)?;
        }
        Ok(())
    }

    pub(super) fn ui_response(
        &mut self,
        header: WireHeader,
        id: UiChallengeId,
        resources: &ParentResources<'_, '_>,
    ) -> Result<()> {
        self.consume_ui_response(header, id, resources.execution.client.now()?)
    }

    /// Apply authenticated activity without losing an already admitted challenge's original
    /// transport correlation. Identical activity preserves the current record and cutoff.
    pub(super) fn ui_activity(&mut self, activity: UiWatchdogActivity, now: Moment) -> Result<()> {
        let watchdog = self.watchdog.as_mut().ok_or(ParentError::WrongPhase)?;
        let previous = watchdog.snapshot().activity;
        watchdog.set_activity(self.run, activity, now)?;
        if previous != activity {
            self.retire_challenge(now)?;
        }
        Ok(())
    }
    /// Record queue admission only; no dispatch evidence or reply authority is created.
    fn queue_challenge_write(&mut self, key: UiChallengeKey, operation: u64) -> Result<()> {
        let challenge = self.challenge.as_mut().ok_or(ParentError::WrongPhase)?;
        if challenge.key != key || challenge.operation.is_some() || operation == 0 {
            return Err(ParentError::WrongPhase.into());
        }
        challenge.operation = Some(operation);
        Ok(())
    }
    /// Called only after the same retained WriteReceipt accepted its actual WriteIssued fact.
    /// Delayed issue observation may complete a retired record, retaining the original cutoff.
    pub(super) fn challenge_issued(
        &mut self,
        key: UiChallengeKey,
        operation: u64,
        at: Moment,
        now: Moment,
    ) -> Result<()> {
        let challenge = self
            .challenge
            .as_mut()
            .filter(|challenge| challenge.key == key)
            .or_else(|| {
                self.retired_challenges.iter_mut().flatten().find(|challenge| challenge.key == key)
            })
            .ok_or(ParentError::StaleRequest)?;
        if challenge.operation != Some(operation)
            || challenge.issued_at.is_some()
            || at < challenge.queued_at
            || at >= challenge.until
            || at > now
        {
            return Err(ParentError::WrongPhase.into());
        }
        challenge.issued_at = Some(at);
        let watchdog = self.watchdog.as_mut().ok_or(ParentError::WrongPhase)?;
        if watchdog.snapshot().pending == Some(key) {
            watchdog.observe(
                self.run,
                UiWatchdogObservation::Dispatched { challenge: key, at },
                now,
            )?;
        }
        Ok(())
    }
    /// Actual checked pre-issue revocation removes retired metadata or permits a current
    /// queued record to retry its same original header. It never establishes UI progress.
    pub(super) fn challenge_revoked(&mut self, key: UiChallengeKey, operation: u64) -> Result<()> {
        if let Some(challenge) = self.challenge.as_mut().filter(|challenge| challenge.key == key) {
            if challenge.operation != Some(operation) || challenge.issued_at.is_some() {
                return Err(ParentError::WrongPhase.into());
            }
            challenge.operation = None;
            return Ok(());
        }
        let slot = self
            .retired_challenges
            .iter_mut()
            .find(|slot| slot.is_some_and(|challenge| challenge.key == key))
            .ok_or(ParentError::StaleRequest)?;
        if slot.as_ref().is_none_or(|challenge| {
            challenge.operation != Some(operation) || challenge.issued_at.is_some()
        }) {
            return Err(ParentError::WrongPhase.into());
        }
        *slot = None;
        Ok(())
    }
    /// Preserve only admitted writes; an effect that never entered the native queue cannot reply.
    fn retire_challenge(&mut self, now: Moment) -> Result<()> {
        self.expire_retired_challenges(now);
        let Some(challenge) = self.challenge else { return Ok(()) };
        if challenge.operation.is_none() {
            self.challenge = None;
            return Ok(());
        }
        let slot = self
            .retired_challenges
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(ParentError::Overloaded)?;
        *slot = Some(challenge);
        self.challenge = None;
        Ok(())
    }
    /// Expired metadata grants no response authority. Keep an outstanding native write's
    /// record until its actual issue/revocation observation, without renewing its deadline.
    fn expire_retired_challenges(&mut self, now: Moment) {
        let pending = self.write.as_ref().map(|write| write.receipt.operation);
        for slot in &mut self.retired_challenges {
            if slot
                .is_some_and(|challenge| now >= challenge.until && challenge.operation != pending)
            {
                *slot = None;
            }
        }
    }
    /// Consume exact original UI processing after activity retirement without renewing or
    /// responding to the current watchdog. Unknown, unissued and expired correlations reject.
    fn consume_ui_response(
        &mut self,
        header: WireHeader,
        id: UiChallengeId,
        now: Moment,
    ) -> Result<()> {
        if let Some(challenge) = self.challenge.filter(|challenge| challenge.key.id == id) {
            self.validate_ui_response(challenge, header, id, now)?;
            self.watchdog.as_mut().ok_or(ParentError::WrongPhase)?.observe(
                self.run,
                UiWatchdogObservation::Responded { challenge: challenge.key },
                now,
            )?;
            self.challenge = None;
            return Ok(());
        }
        let index = self
            .retired_challenges
            .iter()
            .position(|slot| slot.is_some_and(|challenge| challenge.key.id == id))
            .ok_or(ParentError::StaleRequest)?;
        let challenge = self.retired_challenges[index].ok_or(ParentError::WrongPhase)?;
        self.validate_ui_response(challenge, header, id, now)?;
        self.retired_challenges[index] = None;
        tracing::info!(operation="retired_ui_response_consumed",run=?self.run,challenge=?challenge.key,
            "Original issued UI response consumed after native activity retirement");
        Ok(())
    }
    /// Validate only retained correlation; channel authentication remains with receive().
    fn validate_ui_response(
        &self,
        challenge: Challenge,
        header: WireHeader,
        id: UiChallengeId,
        now: Moment,
    ) -> Result<()> {
        if header.run != self.run
            || header.connection != Some(self.connection)
            || challenge.key.run != self.run
            || header.request.is_none()
        {
            return Err(ParentError::ForeignPeer.into());
        }
        if challenge.key.id != id || header.deadline_qpc != challenge.header.deadline_qpc {
            return Err(ParentError::ChangedDuplicate.into());
        }
        let issued = challenge.issued_at.ok_or(ParentError::WrongPhase)?;
        if now < issued {
            return Err(ParentError::WrongPhase.into());
        }
        if now >= challenge.until {
            return Err(sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError::DeadlineExpired.into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "watchdog/tests.rs"]
mod tests;
