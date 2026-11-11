// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Engine publication waits for exact creator status before source recovery or completion.
use super::*;
use std::time::Duration;

impl Launcher {
    /// Every observation is a finite worker operation under the original startup cutoff.
    pub(super) fn observe_offer(&mut self) -> Result<()> {
        let source = self.source.as_ref().ok_or("Activation source absent")?.clone();
        self.activation_after = None;
        self.activation_wake = None;
        self.submit(Task::ObserveOffer, move |context| {
            source.poll(&context).map(Work::OfferObserved)
        })
    }

    /// No response is uncertainty. Only the actual creator's negative reply permits trial recovery.
    pub(super) fn offer_observed(&mut self, outcome: Option<CreationOutcome>) -> Result<()> {
        if let Some(outcome) = outcome {
            self.creator_reply = Some(outcome);
        }
        match outcome {
            None => self.pace_activation(),
            Some(CreationOutcome::Created) => {
                self.completed = true;
                self.choice = Choice::Complete;
                Ok(())
            }
            Some(CreationOutcome::NotCreated) => {
                let now = self.clock.now();
                let until = sugarsubstitute_contracts::lifecycle::ShutdownBudget::new(now)?
                    .settle_until
                    .min(self.until.ok_or("Launcher cutoff absent")?);
                self.recovery_until = Some(until);
                self.recovery_wake = Instant::now()
                    .checked_add(until.checked_duration_since(now).unwrap_or_default());
                self.choice = Choice::Recover;
                Ok(())
            }
            Some(CreationOutcome::Indeterminate) => {
                Err("Native application creation remains uncertain".into())
            }
        }
    }

    /// Reuse the event-loop deadline owner to pace both protocol and native settlement polls.
    pub(super) fn pace_activation(&mut self) -> Result<()> {
        let now = self.clock.now();
        let until =
            if self.closing { self.cleanup_until } else { self.recovery_until.or(self.until) }
                .ok_or("Activation observation cutoff absent")?;
        let after = now.checked_add(
            until.checked_duration_since(now).unwrap_or_default().min(Duration::from_millis(100)),
        )?;
        self.activation_after = Some(after);
        self.activation_wake =
            Instant::now().checked_add(after.checked_duration_since(now).unwrap_or_default());
        Ok(())
    }
}
