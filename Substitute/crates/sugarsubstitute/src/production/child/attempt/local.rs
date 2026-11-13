// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Join actual local never-issued withdrawal with current native absence and finite owner disposal.
use super::*;
use crate::production::child::requests::WithdrawnRequest;
/// Retain genuine withdrawal and actual creating-thread absence independently of remote ACKs.
/// Only the owning native callback can supply absence; publication cannot manufacture it.
pub(in crate::production::child) struct LocalAttemptRetirement {
    identity: MainIdentity,
    original: Moment,
    withdrawn: Option<WithdrawnRequest>,
    native: Option<channel::write::UnissuedAcquisition>,
    absence: Option<MainIdentity>,
    notified: bool,
}
impl LocalAttemptRetirement {
    pub(in crate::production::child) fn complete(&self) -> bool {
        self.notified && self.absence == Some(self.identity)
    }
}
impl ProductionChild {
    /// Transfer genuine never-issued withdrawal into the existing local cleanup transaction.
    /// No Parent operation is sent when no native Acquire could have reached that Parent.
    pub(in crate::production::child) fn begin_local_retirement(&mut self) -> Result<bool> {
        let attempt = self.attempt.as_ref().ok_or("Original cleanup absent")?;
        if attempt.local.is_some() {
            return Ok(true);
        }
        let local = match self.acquire {
            Some(p) => !p.admitted || self.bootstrap.channel_mut().never_issued(p.header)?,
            None => self.acquisition_header.is_none(),
        };
        if !local {
            return Ok(false);
        }
        if self.lease.is_some()
            || self.output.binding.is_some()
            || self.output.pending()
            || self.main.profile().is_some()
            || self.grant_header.is_some()
            || self.grant.is_some()
            || self.main_construction_pending()
            || self.bootstrap.projection_pending()
            || self.bootstrap.channel_mut().writes_pending()
        {
            return Err(
                "Never-issued local retirement conflicts with original remote/native custody"
                    .into(),
            );
        }
        let (withdrawn, native) = match self.acquire {
            Some(p) => {
                let (proof, native) =
                    self.bootstrap.channel_mut().withdraw_acquisition(p.header, self.identity)?;
                (Some(proof), native)
            }
            None => (None, None),
        };
        if let Some(proof) = &withdrawn
            && (self.acquire.is_none_or(|pending| pending.admitted != proof.native_was_admitted())
                || proof.native_was_admitted() != native.is_some())
        {
            return Err("Original native withdrawal admission changed".into());
        }
        let original = self.attempt.as_ref().ok_or("Original cleanup absent")?.original;
        self.attempt.as_mut().ok_or("Original cleanup absent")?.local =
            Some(LocalAttemptRetirement {
                identity: self.identity,
                original,
                withdrawn,
                native,
                absence: None,
                notified: false,
            });
        self.acquire = None;
        Ok(true)
    }
    /// Adopt only the actual requested native absence for this exact never-issued owner.
    /// Original remote retirement flags and control queue remain untouched.
    pub(in crate::production::child) fn record_local_main_absence(
        &mut self,
        observed: MainIdentity,
        construction_pending: bool,
    ) -> Result<bool> {
        let Some(local) = self.attempt.as_mut().and_then(|a| a.local.as_mut()) else {
            return Ok(false);
        };
        if observed != self.identity
            || observed != local.identity
            || construction_pending
            || !self.shutdown.main_absence_pending
            || local.absence.is_some()
            || self.main.profile().is_some()
            || self.output.pending()
            || self.lease.is_some()
        {
            return Err(
                "Local native absence is foreign, duplicate, unrequested or still custodial".into(),
            );
        }
        local.absence = Some(observed);
        self.shutdown.main_absence_pending = false;
        Ok(true)
    }
    /// Publish settlement only after actual absence and the checked original withdrawal join.
    /// This neither sends a Parent ACK nor sets the remote acknowledged/settled flags.
    pub(in crate::production::child) fn advance_local_retirement(&mut self) -> Result<()> {
        let attempt = self.attempt.as_ref().ok_or("Original cleanup absent")?;
        let local = attempt.local.as_ref().ok_or("Original local withdrawal absent")?;
        if local.native.as_ref().is_some_and(|native| {
            native.identity != self.identity
                || local.withdrawn.as_ref().is_none_or(|proof| {
                    proof.header() != native.header || proof.original() != native.original
                })
        }) || local.identity != self.identity
            || local.original != attempt.original
            || local.withdrawn.as_ref().is_some_and(|proof| {
                Some(proof.header()) != self.acquisition_header
                    || self
                        .connection_budget
                        .is_none_or(|budget| budget.until() != proof.original())
            })
        {
            return Err("Original local withdrawal provenance changed".into());
        }
        if local.absence.is_none() {
            if !self.shutdown.main_absence_pending {
                self.push_command(DesktopStartupCommand::ConfirmMainAbsent(self.identity))?;
                self.shutdown.main_absence_pending = true;
            }
            return Ok(());
        }
        if local.notified {
            return Ok(());
        }
        if self.main.pending()
            || self.main_construction_pending()
            || self.main.profile().is_some()
            || self.output.pending()
            || self.acquire.is_some()
            || self.grant.is_some()
            || self.lease.is_some()
            || self.bootstrap.projection_pending()
            || self.bootstrap.channel_mut().writes_pending()
        {
            return Err("Actual local finite/native custody remains after absence".into());
        }
        self.push_event(StartupEvent::new(
            self.identity.launch,
            StartupObservation::ConnectionSettled,
        ))?;
        self.attempt
            .as_mut()
            .and_then(|a| a.local.as_mut())
            .ok_or("Original local proof absent")?
            .notified = true;
        Ok(())
    }
}
