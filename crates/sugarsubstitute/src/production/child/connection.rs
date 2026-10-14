// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow the actual application's configured target into the existing bounded wire buffer.
use super::*;
use sugarsubstitute_application::startup::ConnectionBudgetKind;
use sugarsubstitute_contracts::configuration::ConnectionConfiguration;
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, ReplyOutcome, WireMessage, WirePayload,
};
#[derive(Clone, Copy)]
pub(super) struct PendingAcquire {
    pub(super) header: WireHeader,
    pub(super) original: Moment,
    pub(super) admitted: bool,
}
/// Closed progress facts describe wire ownership, never backend or grant authority.
#[derive(Clone, Copy, Debug)]
pub(super) enum ConnectionStage {
    /// The actual ledger retained the immutable original request and cutoff.
    Queued,
    /// The original channel accepted the exact encoded request for writing.
    Admitted,
    /// The original matching frame transferred to its semantic owner.
    ReplyReceived,
    /// The matching reply decoded a grant; transport metadata grants no native authority.
    Granted,
    /// The matching reply decoded the parent's original failure disposition.
    Failed,
    /// The matching reply decoded the parent's original typed rejection.
    Rejected,
}
impl ConnectionStage {
    /// Emit one closed token at the genuine owner transition without payload or paths.
    pub(super) fn report(self) {
        tracing::info!(operation="child_connection_transition",stage=?self,
            "Original child connection request advanced");
    }
}
impl PendingAcquire {
    /// Reject the unchanged original cutoff before wire effects or reply-loan transfer.
    fn check_current(self, now: Moment) -> std::io::Result<()> {
        if now >= self.original {
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Original connection acquisition budget expired",
            ))
        } else {
            Ok(())
        }
    }
    /// Transfer only this request's exact observed reply; unrelated owners retain theirs.
    /// Matching metadata grants no payload, native grant or readiness authority.
    fn take_matching<T>(
        self,
        now: Moment,
        observed: Option<WireHeader>,
        take: impl FnOnce() -> Option<T>,
    ) -> std::io::Result<Option<T>> {
        self.check_current(now)?;
        Ok(if observed == Some(self.header) { take() } else { None })
    }
}
impl ProductionChild {
    pub(super) fn begin_connection(&mut self, prepared: &PreparedChildStartup) -> Result<()> {
        if self.connection_budget.is_some() || self.acquire.is_some() || self.lease.is_some() {
            return Err("Connection preparation duplicated".into());
        }
        let kind = match prepared.session().configuration().connection() {
            ConnectionConfiguration::Launch(_) => ConnectionBudgetKind::Managed,
            ConnectionConfiguration::Attach(_) => ConnectionBudgetKind::Attached,
        };
        let budget = ConnectionBudget::new(kind, self.client()?.now()?)?;
        self.connection_budget = Some(budget);
        let header = self.bootstrap.channel_mut().prepare_request(budget.until())?;
        self.acquire = Some(PendingAcquire { header, original: budget.until(), admitted: false });
        self.acquisition_header = Some(header);
        ConnectionStage::Queued.report();
        self.send_acquire(prepared)
    }
    pub(super) fn send_acquire(&mut self, prepared: &PreparedChildStartup) -> Result<()> {
        let Some(pending) = self.acquire else { return Ok(()) };
        pending.check_current(self.client()?.now()?)?;
        if pending.admitted {
            return Ok(());
        }
        if self.bootstrap.channel_mut().try_send(
            &WireMessage {
                header: pending.header,
                payload: WirePayload::Acquire {
                    identity: self.identity,
                    target: prepared.session().configuration().connection().into(),
                },
            },
            pending.original,
        )? {
            self.acquire.as_mut().ok_or("Original acquire disappeared")?.admitted = true;
            ConnectionStage::Admitted.report();
        }
        Ok(())
    }
    pub(super) fn receive_acquire(&mut self) -> Result<()> {
        let Some(pending) = self.acquire else { return Ok(()) };
        let now = self.client()?.now()?;
        pending.check_current(now)?;
        if !pending.admitted {
            return Ok(());
        }
        let observed = self.bootstrap.channel_mut().received_header()?;
        let Some(frame) =
            pending.take_matching(now, observed, || self.bootstrap.channel_mut().take_frame())?
        else {
            return Ok(());
        };
        self.grant = Some(frame);
        let frame = FrameCodec::decode(
            self.grant.as_ref().ok_or("Original grant frame absent")?.as_bytes(),
        )?;
        if frame.header != pending.header {
            return Err("Acquire reply differs from original request".into());
        }
        ConnectionStage::ReplyReceived.report();
        match &frame.payload {
            DecodedPayload::Reply(ReplyOutcome::Granted(_)) => ConnectionStage::Granted.report(),
            DecodedPayload::Reply(ReplyOutcome::Failed(_)) => ConnectionStage::Failed.report(),
            DecodedPayload::Reply(ReplyOutcome::Rejected(_)) => ConnectionStage::Rejected.report(),
            _ => {}
        }
        match frame.payload {
            DecodedPayload::Reply(ReplyOutcome::Granted(grant))
                if grant.identity == self.identity =>
            {
                if self.events.iter().all(Option::is_some) {
                    return Err("Startup event slots full before grant".into());
                }
                self.lease = Some(grant.lease);
                self.grant_header = Some(frame.header);
                let origin = grant.origin;
                self.acquire = None;
                self.push_event(StartupEvent::new(
                    self.identity.launch,
                    StartupObservation::ConnectionReady(origin),
                ))
            }
            DecodedPayload::Reply(ReplyOutcome::Pending(_)) => {
                Err("Pending acquire has no final readiness grant".into())
            }
            DecodedPayload::Reply(ReplyOutcome::Failed(code)) => {
                self.acquire_failed(pending.header, attempt::AcquisitionFailure::Failed(code))
            }
            DecodedPayload::Reply(ReplyOutcome::Rejected(code)) => {
                self.acquire_failed(pending.header, attempt::AcquisitionFailure::Rejected(code))
            }
            _ => Err("Acquire reply is not the exact current grant".into()),
        }
    }
    pub(super) fn activity(&mut self, prepared: Option<&PreparedChildStartup>) -> Result<()> {
        if !self.shutdown.closing()
            && let Some(pending) = self.acquire
        {
            pending.check_current(self.client()?.now()?)?;
        }
        self.bootstrap.channel_mut().activity(NonZeroUsize::MIN.saturating_add(31))?;
        if self.shutdown.closing() {
            return self.advance_shutdown();
        }
        if self.phase == Some(sugarsubstitute_application::startup::StartupPhase::WebviewLoading)
            && let Some(budget) = self.frontend_budget
            && self.client()?.now()? >= budget.until()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Original frontend loading budget expired",
            )
            .into());
        }
        if let Some(prepared) = prepared {
            self.send_acquire(prepared)?;
        }
        self.receive_acquire()?;
        self.advance_attempt()?;
        self.advance_control()?;
        self.advance_profile()?;
        self.advance_output()
    }
}

#[cfg(test)]
#[path = "connection/tests.rs"]
mod tests;
