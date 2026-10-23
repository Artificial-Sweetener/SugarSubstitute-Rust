// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report the actual policy-accepted main reveal through the authenticated lifecycle channel.
use super::*;
use std::time::Duration;
use sugarsubstitute_application::startup::StartupPhase;
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, ReplyOutcome, WireMessage, WirePayload,
};
#[cfg(test)]
#[path = "readiness/tests.rs"]
mod tests;

/// One exact finite acknowledgement; native transport and received frame custody stay owned.
pub(super) struct Readiness {
    identity: MainIdentity,
    header: WireHeader,
    until: Moment,
    admitted: bool,
    acknowledged: bool,
    frame: Option<ReceivedLifecycleFrame>,
}
impl Readiness {
    pub(super) fn deadline(&self) -> Option<Moment> {
        (!self.acknowledged).then_some(self.until)
    }
}
impl ProductionChild {
    /// Called only after shared startup policy accepts the current native main reveal.
    pub(super) fn report_main_revealed(
        &mut self,
        identity: MainIdentity,
        prepared: Option<&PreparedChildStartup>,
    ) -> Result<()> {
        let prepared = prepared.ok_or("Main reveal has no prepared startup")?;
        if self.shutdown.closing()
            || identity != self.identity
            || prepared.session().launch() != identity.launch
            || prepared.session().navigation() != identity.navigation
            || prepared.session().phase() != StartupPhase::Running
            || !self.main.profile().is_some_and(|profile| profile.authorized())
            || self
                .readiness
                .as_ref()
                .is_some_and(|previous| previous.identity == identity || !previous.acknowledged)
        {
            return Err("Main reveal is not a current authorized running frontend".into());
        }
        let until = self.client()?.now()?.checked_add(Duration::from_secs(30))?;
        let header = self.bootstrap.channel_mut().prepare_request(until)?;
        self.readiness = Some(Readiness {
            identity,
            header,
            until,
            admitted: false,
            acknowledged: false,
            frame: None,
        });
        Ok(())
    }

    /// Keep this acknowledgement independent of shutdown controls and their original cutoffs.
    /// On close, ordinary admission stops and the existing channel retirement drains replies.
    pub(super) fn advance_readiness(&mut self) -> Result<()> {
        if self.shutdown.closing() {
            if let Some(readiness) = &self.readiness
                && !readiness.admitted
            {
                self.bootstrap.channel_mut().withdraw_prepared(readiness.header)?;
                self.readiness = None;
            }
            return Ok(());
        }
        let Some(readiness) = &mut self.readiness else { return Ok(()) };
        if readiness.acknowledged {
            return Ok(());
        }
        let channel = self.bootstrap.channel_mut();
        if !readiness.admitted {
            readiness.admitted = channel.try_send(
                &WireMessage {
                    header: readiness.header,
                    payload: WirePayload::MainRevealed(readiness.identity),
                },
                readiness.until,
            )?;
        }
        if readiness.admitted && channel.received_header()? == Some(readiness.header) {
            readiness.frame = channel.take_frame();
            let frame = readiness.frame.as_ref().ok_or("Main reveal reply disappeared")?;
            let decoded = FrameCodec::decode(frame.as_bytes())?;
            if decoded.header != readiness.header
                || !matches!(decoded.payload, DecodedPayload::Reply(ReplyOutcome::Acknowledged))
            {
                return Err("Main reveal was not acknowledged by its coordinator".into());
            }
            channel.consume_reply(readiness.header)?;
            readiness.acknowledged = true;
            readiness.frame = None;
            crate::production::startup_readiness_diagnostic::Record::new(readiness.identity).emit();
        }
        Ok(())
    }
}
