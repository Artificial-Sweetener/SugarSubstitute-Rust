// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve one original UI challenge and count only actual native issue and response.
use super::types::WritePurpose;
use super::{ParentError, ParentLifecycle, ParentResources, Result};
use sugarsubstitute_contracts::lifecycle::{
    UI_SUSPICION_THRESHOLD, UiChallengeId, UiWatchdogEffect, UiWatchdogObservation,
};
use sugarsubstitute_supervision::lifecycle_protocol::{WireHeader, WirePayload};

impl ParentLifecycle {
    pub(super) fn send_challenge(&mut self, resources: &ParentResources<'_, '_>) -> Result<()> {
        if !self.registered || self.cleanup.is_some() {
            return Ok(());
        }
        let now = resources.execution.client.now()?;
        let watchdog = self.watchdog.as_mut().ok_or(ParentError::WrongPhase)?;
        if self
            .challenge
            .as_ref()
            .is_some_and(|(key, _, _)| watchdog.snapshot().pending != Some(*key))
        {
            self.challenge = None;
        }
        let effect = watchdog.poll(now)?;
        if self.challenge.is_none()
            && let Some(UiWatchdogEffect::Challenge { challenge }) = effect
        {
            let until = now.checked_add(UI_SUSPICION_THRESHOLD)?;
            let counter = resources.execution.client.encode_counter_deadline(until)?;
            self.challenge = Some((
                challenge,
                WireHeader {
                    run: self.run,
                    connection: Some(self.connection),
                    request: None,
                    deadline_qpc: counter.ticks(),
                },
                until,
            ));
        }
        let Some((key, header, until)) = self.challenge else {
            return Ok(());
        };
        if watchdog.snapshot().phase
            != Some(sugarsubstitute_application::supervision::UiChallengePhase::Queued)
        {
            return Ok(());
        }
        watchdog.validate_dispatch(self.run, key)?;
        self.write_message(
            resources,
            header,
            WirePayload::UiChallenge(key.id),
            until,
            WritePurpose::Challenge(key),
        )?;
        Ok(())
    }

    pub(super) fn ui_response(
        &mut self,
        header: WireHeader,
        id: UiChallengeId,
        resources: &ParentResources<'_, '_>,
    ) -> Result<()> {
        let (key, original, _) = self.challenge.ok_or(ParentError::WrongPhase)?;
        if key.id != id || header.deadline_qpc != original.deadline_qpc {
            return Err(ParentError::ChangedDuplicate.into());
        }
        self.watchdog.as_mut().ok_or(ParentError::WrongPhase)?.observe(
            self.run,
            UiWatchdogObservation::Responded { challenge: key },
            resources.execution.client.now()?,
        )?;
        self.challenge = None;
        Ok(())
    }
}
