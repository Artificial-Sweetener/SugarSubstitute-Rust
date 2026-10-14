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
/// The original acquire owner retains one reply through cancellation; matching metadata
/// neither accepts its disposition nor grants output, Main or settlement authority.
#[derive(Debug, Eq, PartialEq)]
enum RetiringAcquireFrame {
    Awaiting,
    Retained,
    Transferred,
}
/// Select progression without granting settlement or replacing any stored cutoff.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum ActivityRoute {
    Shutdown,
    AttemptCleanup,
    Startup,
}
impl ActivityRoute {
    /// An existing cleanup owns its original budget; an expired startup budget must
    /// not prevent actual control/attempt retirement after the initiating failure.
    pub(super) fn select(
        closing: bool,
        attempt: Option<&attempt::AttemptCleanup>,
        phase: Option<sugarsubstitute_application::startup::StartupPhase>,
        frontend: Option<FrontendBudget>,
        now: Moment,
    ) -> std::io::Result<Self> {
        if closing {
            return Ok(Self::Shutdown);
        }
        if attempt.is_some() {
            return Ok(Self::AttemptCleanup);
        }
        if phase == Some(sugarsubstitute_application::startup::StartupPhase::WebviewLoading)
            && frontend.is_some_and(|budget| now >= budget.until())
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Original frontend loading budget expired",
            ));
        }
        Ok(Self::Startup)
    }
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
    /// The authentic nonfinal managed source was accepted without frontend readiness.
    OutputReserved,
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
    /// Retain the original reply in its existing acquisition slot. Cancellation does not
    /// move it through a different owner's failure slot or replace an already retained loan.
    /// The original ledger performs cutoff/disposition checks before actual consumption.
    fn retain_retiring<T>(
        self,
        retained: &mut Option<T>,
        observed: Option<WireHeader>,
        take: impl FnOnce() -> Option<T>,
    ) -> std::io::Result<RetiringAcquireFrame> {
        if !self.admitted {
            return Err(std::io::Error::other("Original acquire was not admitted"));
        }
        if retained.is_some() {
            return Ok(RetiringAcquireFrame::Retained);
        }
        if observed != Some(self.header) {
            return Ok(RetiringAcquireFrame::Awaiting);
        }
        *retained = take();
        if retained.is_none() {
            return Err(std::io::Error::other("Original retiring acquire frame disappeared"));
        }
        Ok(RetiringAcquireFrame::Transferred)
    }
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
        self.bootstrap.channel_mut().acquisition(header)?;
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
                let (lease, origin) = (grant.lease, grant.origin);
                self.adopt_output(lease, pending.original, true)?;
                self.lease = Some(lease);
                self.grant_header = Some(pending.header);
                self.acquire = None;
                self.push_event(StartupEvent::new(
                    self.identity.launch,
                    StartupObservation::ConnectionReady(origin),
                ))
            }
            DecodedPayload::Reply(ReplyOutcome::Pending(lease)) => {
                self.adopt_output(lease, pending.original, false)?;
                self.bootstrap.channel_mut().consume_progress(pending.header)?;
                self.grant = None;
                ConnectionStage::OutputReserved.report();
                Ok(())
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
    /// Drain the original nonfinal/final acquisition during cancellation without starting
    /// output, publishing readiness or renewing its deadline. Errors retain the real frame.
    fn retire_acquire(&mut self) -> Result<()> {
        let Some(pending) = self.acquire else { return Ok(()) };
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
            self.grant.as_ref().ok_or("Original retiring acquire frame missing")?.as_bytes(),
        )?;
        if decoded.header != pending.header {
            return Err("Original retiring acquisition header changed".into());
        }
        let DecodedPayload::Reply(reply) = decoded.payload else {
            return Err("Original retiring acquisition reply changed".into());
        };
        match reply {
            ReplyOutcome::Pending(lease) => {
                if self.output.binding.is_some_and(|binding| {
                    binding.lease != lease || retained != RetiringAcquireFrame::Retained
                }) {
                    return Err("Retiring provisional output source duplicated or changed".into());
                }
                self.output.binding = Some(crate::production_output::OutputBinding {
                    resource: ResourceKey {
                        owner: self.run.owner,
                        id: lease.resource,
                        generation: lease.generation,
                    },
                    run: self.run,
                    connection: pending.header.connection.ok_or("Original connection missing")?,
                    lease,
                });
                self.bootstrap.channel_mut().consume_progress(pending.header)?;
            }
            ReplyOutcome::Granted(grant) if grant.identity == self.identity => {
                if self.output.binding.is_some_and(|binding| binding.lease != grant.lease) {
                    return Err("Retiring final output source changed".into());
                }
                if self.main_construction_pending() || self.main.profile().is_some() {
                    return Err("Late Grant still has original Main construction custody".into());
                }
                self.push_command(DesktopStartupCommand::ConfirmMainAbsent(self.identity))?;
                self.shutdown.main_absence_pending = true;
                self.bootstrap.channel_mut().consume_reply(pending.header)?;
                self.main.request_consumed = true;
                self.grant_header = Some(pending.header);
                self.acquire = None;
            }
            ReplyOutcome::Failed(_) | ReplyOutcome::Rejected(_) | ReplyOutcome::Cancelled => {
                self.bootstrap.channel_mut().consume_reply(pending.header)?;
                self.acquire = None;
            }
            _ => return Err("Retiring acquisition has no final disposition".into()),
        }
        self.grant = None;
        Ok(())
    }
    pub(super) fn activity(&mut self, prepared: Option<&PreparedChildStartup>) -> Result<()> {
        if !self.shutdown.closing()
            && self.attempt.is_none()
            && let Some(pending) = self.acquire
        {
            pending.check_current(self.client()?.now()?)?;
        }
        self.bootstrap.channel_mut().activity(NonZeroUsize::MIN.saturating_add(31))?;
        if self.output.closing() {
            self.advance_output()?;
        }
        let route = ActivityRoute::select(
            self.shutdown.closing(),
            self.attempt.as_ref(),
            self.phase,
            self.frontend_budget,
            self.client()?.now()?,
        );
        let route = match route {
            Ok(route) => route,
            Err(cause) => {
                self.publish_frontend_timeout(cause)?;
                return Ok(());
            }
        };
        match route {
            ActivityRoute::Shutdown => {
                self.retire_acquire()?;
                return self.advance_shutdown();
            }
            ActivityRoute::AttemptCleanup => {
                self.retire_acquire()?;
                self.advance_control()?;
                return self.advance_attempt();
            }
            ActivityRoute::Startup => {}
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
