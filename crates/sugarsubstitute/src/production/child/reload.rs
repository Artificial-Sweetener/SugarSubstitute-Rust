// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replace one current native frontend through actual retirement and the same retained backend lease.
use super::*;
use std::time::Duration;
use sugarsubstitute_application::startup::StartupPhase;
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, ReplyOutcome, WireMessage, WirePayload,
};

#[derive(Clone, Copy)]
enum Stage {
    Retiring,
    Request { header: WireHeader, admitted: bool },
    Granted,
}
/// Fixed correlation only. Native, profile, source and backend authority stay with their owners.
pub(super) struct FrontendReload {
    previous: MainIdentity,
    next: MainIdentity,
    lease: LeaseReference,
    until: Moment,
    origin: Option<sugarsubstitute_contracts::configuration::LoopbackOrigin>,
    stage: Stage,
}
impl FrontendReload {
    pub(super) const fn until(&self) -> Moment {
        self.until
    }
    /// Retain finite retirement/write admission; an admitted readiness reply waits for lifecycle.
    pub(super) fn deadline(&self) -> Option<Moment> {
        match self.stage {
            Stage::Retiring | Stage::Request { admitted: false, .. } => Some(self.until),
            Stage::Request { admitted: true, .. } | Stage::Granted => None,
        }
    }
    pub(super) const fn granted(&self) -> bool {
        matches!(self.stage, Stage::Granted)
    }
}
impl ProductionChild {
    /// Coalesce this exact native request without closing the backend/output/capture owners.
    pub(super) fn request_reload(&mut self, identity: MainIdentity) -> Result<()> {
        if self.shutdown.closing() {
            return Ok(());
        }
        if let Some(reload) = &self.reload {
            return if reload.previous == identity {
                Ok(())
            } else {
                Err("Frontend reload belongs to another Main".into())
            };
        }
        if identity != self.identity
            || self.phase != Some(StartupPhase::Running)
            || self.attempt.is_some()
            || self.main.pending()
            || !self.main.profile().is_some_and(profile_transaction::ProfileTransaction::authorized)
        {
            return Err("Frontend reload requires the actual current running Main".into());
        }
        let lease = self
            .running_backend
            .as_ref()
            .ok_or("Running backend absent")?
            .reload_lease(identity)?;
        let next = MainIdentity {
            run: identity.run,
            launch: LaunchId::new(identity.launch.get().checked_add(1).ok_or("Launch exhausted")?)?,
            navigation: Generation::new(
                identity.navigation.get().checked_add(1).ok_or("Navigation exhausted")?,
            )?,
        };
        let until = self.client()?.now()?.checked_add(Duration::from_secs(15))?;
        self.reload = Some(FrontendReload {
            previous: identity,
            next,
            lease,
            until,
            stage: Stage::Retiring,
            origin: None,
        });
        self.push_command(DesktopStartupCommand::RetireMain {
            identity,
            reason: sugarsubstitute_desktop::MainRetirementReason::FrontendReload,
        })
    }
    /// Retain late closed-operation replies for disposal, never fresh Main admission.
    /// Already-admitted writes keep their original order; a current late grant requires
    /// actual native absence before Root can settle its fresh profile authorization.
    pub(super) fn retire_reload_reply(&mut self) -> Result<()> {
        if !self.shutdown.closing() {
            return Ok(());
        }
        let Some(reload) = &self.reload else {
            return Ok(());
        };
        let (header, admitted) = match reload.stage {
            Stage::Request { header, admitted } => (header, admitted),
            Stage::Granted => (self.grant_header.ok_or("Reload grant header absent")?, true),
            Stage::Retiring => return Ok(()),
        };
        if !admitted {
            self.bootstrap.channel_mut().withdraw_prepared(header)?;
            self.reload = None;
            return Ok(());
        }
        let (next, lease, origin) = (reload.next, reload.lease, reload.origin);
        self.bootstrap.channel_mut().retire_frontend_reload(header)?;
        if self.grant.is_none() && self.bootstrap.channel_mut().received_header()? == Some(header) {
            self.grant = self.bootstrap.channel_mut().take_frame();
        }
        let Some(frame) = &self.grant else {
            return Ok(());
        };
        let decoded = FrameCodec::decode(frame.as_bytes())?;
        if decoded.header != header {
            return Err("Retiring reload frame changed".into());
        }
        match decoded.payload {
            DecodedPayload::Reply(ReplyOutcome::Reloaded(context))
                if context.identity == next
                    && context.lease == lease
                    && Some(context.origin) == origin =>
            {
                if !self.main.construction_pending()
                    && self.main.profile().is_none()
                    && !self.shutdown.main_absence_pending
                {
                    self.push_command(DesktopStartupCommand::ConfirmMainAbsent(next))?;
                    self.shutdown.main_absence_pending = true;
                }
            }
            DecodedPayload::Reply(
                ReplyOutcome::Cancelled | ReplyOutcome::Rejected(_) | ReplyOutcome::Failed(_),
            ) => (),
            _ => return Err("Retiring reload disposition changed".into()),
        }
        self.bootstrap.channel_mut().consume_retired_reload(header)?;
        self.grant = None;
        self.reload = None;
        Ok(())
    }
    /// The composing owner calls after genuine child/native progression; no unrelated completion is required.
    /// Every pending write retains the original header; close prevents any new-generation effect.
    pub(in crate::production) fn advance_reload(
        &mut self,
        prepared: &mut PreparedChildStartup,
    ) -> Result<()> {
        if self.shutdown.closing() {
            return Ok(());
        }
        let Some(reload) = &self.reload else {
            return Ok(());
        };
        let now = self.client()?.now()?;
        if reload.deadline().is_some_and(|until| now >= until) {
            return Err("Original frontend reload operation expired".into());
        }
        if matches!(reload.stage, Stage::Granted) {
            return Ok(());
        }
        if matches!(reload.stage, Stage::Retiring) {
            if !self.shutdown.main_retired || !self.shutdown.main_retirement_ack {
                return Ok(());
            }
            let (previous, next) = (reload.previous, reload.next);
            let mut input = self.main.take_reusable_input()?;
            if let Err(cause) = prepared.restore_main_input(&mut input) {
                self.main.restore_reusable_input(&mut input)?;
                return Err(cause);
            }
            prepared.session_mut().reload_frontend(next.launch, next.navigation)?;
            self.running_backend
                .as_mut()
                .ok_or("Running backend absent")?
                .reload_main(previous, next)?;
            self.panel_deliveries.clear();
            self.identity = next;
            self.phase = Some(prepared.session().phase());
            self.lease = Some(self.reload.as_ref().ok_or("Reload absent")?.lease);
            self.main = main::MainPreparation::new(self.main.settings);
            self.grant_header = None;
            self.shutdown.main_retired = false;
            self.shutdown.main_retirement_ack = false;
            self.shutdown.main_absence_pending = false;
            let until = self.client()?.now()?.checked_add(Duration::from_secs(30))?;
            let header = self.bootstrap.channel_mut().prepare_request(until)?;
            self.bootstrap.channel_mut().frontend_reload(header)?;
            let reload = self.reload.as_mut().ok_or("Reload absent")?;
            reload.until = until;
            reload.origin = prepared.session().connection_origin();
            reload.stage = Stage::Request { header, admitted: false };
        }
        let reload = self.reload.as_ref().ok_or("Reload absent")?;
        let Stage::Request { header, admitted } = reload.stage else {
            return Ok(());
        };
        let (until, next, lease) = (reload.until, reload.next, reload.lease);
        if !admitted {
            if !self.bootstrap.channel_mut().try_send(
                &WireMessage { header, payload: WirePayload::Reload { lease, identity: next } },
                until,
            )? {
                return Ok(());
            }
            self.reload.as_mut().ok_or("Reload absent")?.stage =
                Stage::Request { header, admitted: true };
        }
        if self.bootstrap.channel_mut().received_header()? != Some(header) {
            return Ok(());
        }
        self.grant = self.bootstrap.channel_mut().take_frame();
        let frame =
            FrameCodec::decode(self.grant.as_ref().ok_or("Reload reply absent")?.as_bytes())?;
        let DecodedPayload::Reply(ReplyOutcome::Reloaded(context)) = frame.payload else {
            return Err("Reload did not receive current frontend authorization".into());
        };
        if frame.header != header
            || context.identity != next
            || context.lease != lease
            || Some(context.origin) != prepared.session().connection_origin()
        {
            return Err("Reload authorization changed identity, lease or origin".into());
        }
        let origin = context.origin;
        #[cfg(feature = "native-composition")]
        {
            self.canvas_backend = Some(crate::startup::CanvasBackend {
                identity: next,
                lease,
                origin,
                peer: context.backend_peer,
            });
        }
        self.grant_header = Some(header);
        self.reload.as_mut().ok_or("Reload absent")?.stage = Stage::Granted;
        self.push_event(StartupEvent::new(next.launch, StartupObservation::ConnectionReady(origin)))
    }
}

#[cfg(test)]
#[path = "reload/tests.rs"]
mod tests;
