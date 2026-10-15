// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original acquisition correlation through failed-attempt native and backend settlement.
use super::*;
use connection::{PendingAcquire, RetiringAcquireFrame};
use requests::Purpose;
use sugarsubstitute_contracts::startup::{StartupFailure, StartupFailureCode};
use sugarsubstitute_supervision::lifecycle_protocol::{DecodedPayload, FrameCodec, ReplyOutcome};
impl ProductionChild {
    /// Route only the actual original acquisition cutoff through bounded Failed(TimedOut).
    /// Capture owners observe their batch first; no late ordinary disposition grants authority.
    pub(super) fn expire_acquisition(&mut self) -> Result<bool> {
        let Some(pending) = self.acquire else {
            return Ok(false);
        };
        if self.shutdown.closing() || self.attempt.is_some() {
            return Ok(false);
        }
        if pending.retiring {
            return Ok(true);
        }
        let Err(cause) = pending.check_current(self.client()?.now()?) else {
            return Ok(false);
        };
        self.publish_acquisition_expiry(Box::new(cause))?;
        Ok(true)
    }
    /// Publish only the actual acquisition owner's direct cutoff cause, preserving its box.
    pub(super) fn publish_acquisition_expiry(&mut self, cause: Box<dyn Error>) -> Result<()> {
        let pending = self.acquire.ok_or("Original acquisition owner absent")?;
        if pending.retiring {
            return Ok(());
        }
        if self.events.iter().all(Option::is_some) {
            if self.failure.is_none() {
                self.failure = Some(cause);
            }
            return Err("Acquisition failure event slots full; original cause retained".into());
        }
        if pending.admitted {
            self.bootstrap.channel_mut().retire_request(pending.header, Purpose::Acquisition)?;
        }
        self.acquire.as_mut().ok_or("Original acquisition disappeared")?.retiring = true;
        self.push_event(StartupEvent::new(
            self.identity.launch,
            StartupObservation::Failed(StartupFailure::new(StartupFailureCode::TimedOut, cause)),
        ))?;
        self.failed = true;
        Ok(())
    }
    /// Handle only a direct real ledger rejection for this exact acquisition cutoff.
    /// A provisional pull qualifies only when its original wire cutoff equals Acquire's.
    pub(super) fn recover_acquisition_expiry(&mut self, cause: Box<dyn Error>) -> Result<()> {
        let (header, original, purpose) = if let Some(expiry) =
            cause.downcast_ref::<requests::RequestExpiry>()
        {
            (expiry.header, expiry.original, expiry.purpose)
        } else if let Some(expiry) = cause.downcast_ref::<channel::expiry::NativeWriteExpiry>() {
            (expiry.header, expiry.original, expiry.purpose)
        } else {
            return Err(cause);
        };
        let Some(pending) = self.acquire else {
            return Err(cause);
        };
        let exact = match purpose {
            Purpose::Acquisition => header == pending.header,
            Purpose::OutputPull => {
                self.output.pending.is_some_and(|p| p.0 == header && p.1 == pending.original)
            }
            Purpose::Other | Purpose::BackendRetry => false,
        };
        if self.shutdown.closing()
            || self.attempt.is_some()
            || pending.retiring
            || pending.header.run != self.run
            || header.run != self.run
            || original != pending.original
            || !exact
        {
            return Err(cause);
        }
        self.publish_acquisition_expiry(cause)
    }
    /// Fence exact admitted original correlations while preserving their slots and real write token.
    /// Prepared requests keep the existing distinct local withdrawal authority.
    pub(super) fn fence_acquisition_retirement(&mut self) -> Result<()> {
        if let Some(pending) = self.acquire {
            if pending.admitted {
                self.bootstrap
                    .channel_mut()
                    .retire_request(pending.header, Purpose::Acquisition)?;
            }
            self.acquire.as_mut().ok_or("Original acquisition disappeared")?.retiring = true;
        }
        if let Some((header, _)) = self.output.pending
            && !self.bootstrap.channel_mut().request_prepared(header)?
        {
            self.bootstrap.channel_mut().retire_request(header, Purpose::OutputPull)?;
        }
        Ok(())
    }
    /// Validate an actual retired acquisition frame for disposal only. Its original
    /// correlation survives until the authenticated full SettleAttempt acknowledgement.
    pub(super) fn dispose_retiring_acquire(&mut self, pending: PendingAcquire) -> Result<()> {
        if !pending.admitted {
            return Ok(());
        }
        let observed = if self.grant.is_some() {
            None
        } else {
            self.bootstrap.channel_mut().received_header()?
        };
        let retained = pending.retain_retiring(&mut self.grant, observed, || {
            self.bootstrap.channel_mut().take_frame()
        })?;
        if retained == RetiringAcquireFrame::Awaiting {
            return Ok(());
        }
        let decoded = FrameCodec::decode(
            self.grant.as_ref().ok_or("Original retiring acquisition frame absent")?.as_bytes(),
        )?;
        if decoded.header != pending.header {
            return Err("Retiring acquisition header changed".into());
        }
        match decoded.payload {
            DecodedPayload::Reply(ReplyOutcome::Pending(lease)) => {
                if self.output.binding.is_some() {
                    return Err("Retiring acquisition progress duplicated".into());
                }
                self.output.binding = Some(crate::production_output::OutputBinding {
                    resource: ResourceKey {
                        owner: self.run.owner,
                        id: lease.resource,
                        generation: lease.generation,
                    },
                    run: self.run,
                    connection: pending.header.connection.ok_or("Original connection absent")?,
                    lease,
                });
                self.bootstrap.channel_mut().disposed_progress(pending.header)?;
            }
            DecodedPayload::Reply(ReplyOutcome::Granted(grant))
                if grant.identity == self.identity =>
            {
                if self.main_construction_pending()
                    || self.main.profile().is_some()
                    || self.output.binding.is_some_and(|binding| binding.lease != grant.lease)
                {
                    return Err("Late grant differs or still owns Main construction".into());
                }
            }
            DecodedPayload::Reply(
                ReplyOutcome::Failed(_) | ReplyOutcome::Rejected(_) | ReplyOutcome::Cancelled,
            ) => {}
            _ => return Err("Retiring acquisition disposition changed".into()),
        }
        self.grant = None;
        Ok(())
    }
    /// Dispose only semantic output correlation after real full settlement and ended finite custody.
    /// Actual returned frame guards were validated and released by the existing closed owner.
    pub(super) fn finish_settled_output(&mut self) -> Result<()> {
        if self.output.physical_pending() {
            return Err("Original output worker/frame remains".into());
        }
        if let Some((header, _)) = self.output.pending {
            self.output
                .owner_mut()
                .ok_or("Original closed output owner absent")?
                .finish_retiring_pull(header)?;
            self.output.pending = None;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "acquisition_retirement_tests.rs"]
mod tests;
