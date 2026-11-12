// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route real pipe challenges through Desktop's native command and callback boundary.
use super::*;
use sugarsubstitute_contracts::lifecycle::UiChallengeKey;
use sugarsubstitute_desktop::DesktopStartupCommand;
use sugarsubstitute_supervision::lifecycle_protocol::{DecodedFrame, DecodedPayload};

impl ChildChannel {
    /// Return one actual native UI command after validating its original local cutoff.
    /// Only Desktop's subsequent UiChallengeProcessed observation permits a response.
    /// # Errors
    /// Retains clock failures; expired diagnostic work emits no command.
    pub(in super::super) fn take_ui_command(&mut self) -> Result<Option<DesktopStartupCommand>> {
        if !self.admission.open() || self.ui.is_none() {
            return Ok(None);
        }
        let now =
            self.client.as_ref().ok_or("Original lifecycle execution client missing")?.now()?;
        Ok(self
            .ui
            .as_mut()
            .ok_or("Original UI exchange missing")?
            .dispatch(now)?
            .map(DesktopStartupCommand::ProcessUiChallenge))
    }

    /// Deliver the unchanged key returned by Desktop's actual native UI loop.
    /// Its caller supplies UiChallengeProcessed from the native loop.
    /// # Errors
    /// Rejects foreign, duplicate or undispatched callbacks. Expired probes grant no response.
    pub(in super::super) fn ui_processed(&mut self, key: UiChallengeKey) -> Result<()> {
        if !self.admission.open() {
            return Ok(());
        }
        let now =
            self.client.as_ref().ok_or("Original lifecycle execution client missing")?.now()?;
        self.ui.as_mut().ok_or("Original UI exchange missing")?.processed(key, now)?;
        Ok(())
    }

    /// Borrow the original challenge wake deadline without renewing its interval.
    pub(in super::super) fn ui_deadline(&self) -> Option<Moment> {
        if self.admission.open() {
            self.ui.as_ref().and_then(UiChallengeExchange::deadline)
        } else {
            None
        }
    }

    /// Decode the same actual authenticated server frame once into the existing clock epoch.
    pub(super) fn receive_ui_challenge(&mut self, frame: &DecodedFrame<'_>) -> Result<()> {
        let DecodedPayload::UiChallenge(id) = frame.payload else {
            return Err("Original UI challenge payload changed".into());
        };
        let binding = self.binding().ok_or("Original UI challenge binding missing")?;
        let counter = CounterDeadline::new(frame.header.deadline_qpc, binding.counter_frequency())?;
        let client = self.client.as_ref().ok_or("Original lifecycle execution client missing")?;
        let original = client.decode_live_counter_deadline(counter)?;
        let now = client.now()?;
        let ui = self.ui.as_mut().ok_or("Original UI exchange missing")?;
        ui.expire(self.requests.as_mut().ok_or("Original request ledger missing")?, now)?;
        ui.receive(frame.header, id, original.unwrap_or(now), now)?;
        Ok(())
    }

    /// Attempt bounded response admission after the genuine native callback.
    /// Busy transport preserves the identical response; ACK keeps its ledger slot until handled.
    pub(super) fn advance_ui_response(&mut self) -> Result<()> {
        let now =
            self.client.as_ref().ok_or("Original lifecycle execution client missing")?.now()?;
        let Some(response) = self
            .ui
            .as_mut()
            .ok_or("Original UI exchange missing")?
            .response(self.requests.as_mut().ok_or("Original request ledger missing")?, now)?
        else {
            return Ok(());
        };
        if self.try_send(
            &WireMessage { header: response.header, payload: WirePayload::UiResponse(response.id) },
            response.original,
        )? {
            self.ui.as_mut().ok_or("Original UI exchange missing")?.admitted(response.header)?;
        }
        Ok(())
    }
}
