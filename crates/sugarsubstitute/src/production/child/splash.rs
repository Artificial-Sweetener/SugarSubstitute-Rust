// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain a complete covered native frame until the authenticated Root confirms its hide.
use super::*;
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, ReplyOutcome, WireMessage, WirePayload,
};

#[derive(Clone, Copy)]
struct Pending {
    identity: MainIdentity,
    header: WireHeader,
    original: Moment,
    admitted: bool,
}
/// Keep the original request and received native loan through semantic failure.
#[derive(Default)]
pub(super) struct SplashHandoff {
    submitted: bool,
    released: bool,
    pending: Option<Pending>,
    reply: Option<ReceivedLifecycleFrame>,
}
impl SplashHandoff {
    /// Return the unchanged original cutoff while actual hide acknowledgment is pending.
    pub(super) fn deadline(&self) -> Option<Moment> {
        self.pending.map(|pending| pending.original)
    }
}
impl ProductionChild {
    /// Accept only the actual native callback after complete covered pixels and controls.
    /// An authenticated connection or a prepared raster alone cannot call this transition.
    pub(in crate::production) fn entry_splash_frame_presented(&mut self) -> Result<()> {
        if self.shutdown.closing() || !self.prepared {
            return Err("Covered splash submission has no active prepared child".into());
        }
        if self.splash.submitted {
            return Ok(());
        }
        let original =
            self.bootstrap.original_deadline().ok_or("Original splash admission absent")?;
        let header = self.bootstrap.channel_mut().prepare_request(original)?;
        self.splash.pending =
            Some(Pending { identity: self.identity, header, original, admitted: false });
        self.splash.submitted = true;
        self.advance_splash_handoff()
    }
    /// Allow actual ShowWindow only after the exact authenticated original hide reply.
    pub(in crate::production) fn entry_splash_released(&self) -> bool {
        self.splash.released && !self.shutdown.closing()
    }
    /// Advance through the existing channel; bounded contention preserves the original request.
    pub(super) fn advance_splash_handoff(&mut self) -> Result<()> {
        let Some(mut pending) = self.splash.pending else { return Ok(()) };
        if self.shutdown.closing() {
            return Ok(());
        }
        if pending.identity != self.identity || self.client()?.now()? >= pending.original {
            return Err("Original entry splash handoff expired or changed identity".into());
        }
        if !pending.admitted {
            pending.admitted = self.bootstrap.channel_mut().try_send(
                &WireMessage {
                    header: pending.header,
                    payload: WirePayload::SplashPrepared(pending.identity),
                },
                pending.original,
            )?;
            self.splash.pending = Some(pending);
        }
        if !pending.admitted {
            return Ok(());
        }
        if self.consume_splash_handoff(pending)? {
            self.splash.released = true;
        }
        Ok(())
    }

    /// Drain an already admitted hide reply during shutdown without showing the child.
    /// Only a never-admitted request can be withdrawn; native loans stay retained on error.
    pub(super) fn retire_splash_handoff(&mut self) -> Result<()> {
        let Some(pending) = self.splash.pending else {
            return Ok(());
        };
        if !pending.admitted {
            self.bootstrap.channel_mut().withdraw_prepared(pending.header)?;
            self.splash.pending = None;
            return Ok(());
        }
        self.consume_splash_handoff(pending)?;
        Ok(())
    }

    /// Retain and consume only the exact reply through its original channel ledger.
    fn consume_splash_handoff(&mut self, pending: Pending) -> Result<bool> {
        if self.splash.reply.is_none()
            && self.bootstrap.channel_mut().received_header()? == Some(pending.header)
        {
            self.splash.reply = self.bootstrap.channel_mut().take_frame();
        }
        if let Some(frame) = &self.splash.reply {
            let decoded = FrameCodec::decode(frame.as_bytes())?;
            validate_release(pending.header, decoded.header, &decoded.payload)?;
            self.bootstrap.channel_mut().consume_reply(pending.header)?;
            self.splash.pending = None;
            self.splash.reply = None;
            return Ok(true);
        }
        Ok(false)
    }
}
/// Match the actual request ledger's original header before allowing native visibility.
fn validate_release(
    expected: WireHeader,
    actual: WireHeader,
    payload: &DecodedPayload<'_>,
) -> Result<()> {
    if actual != expected || !matches!(payload, DecodedPayload::Reply(ReplyOutcome::Acknowledged)) {
        return Err("Entry splash release differs from its original authenticated request".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_splash_reply_requires_exact_run_connection_request_and_cutoff() -> Result<()> {
        let expected = WireHeader {
            run: super::super::test_support::run()?,
            connection: Some(ConnectionId::new(1)?),
            request: Some(RequestId::new(3)?),
            deadline_qpc: 100,
        };
        let payload = DecodedPayload::Reply(ReplyOutcome::Acknowledged);
        validate_release(expected, expected, &payload)?;
        for actual in [
            WireHeader { connection: Some(ConnectionId::new(2)?), ..expected },
            WireHeader { request: Some(RequestId::new(4)?), ..expected },
            WireHeader { deadline_qpc: 101, ..expected },
            WireHeader {
                run: RunKey { generation: RunGeneration::new(2)?, ..expected.run },
                ..expected
            },
        ] {
            assert!(validate_release(expected, actual, &payload).is_err());
        }
        assert!(
            validate_release(expected, expected, &DecodedPayload::Reply(ReplyOutcome::Cancelled))
                .is_err()
        );
        assert!(
            validate_release(
                expected,
                expected,
                &DecodedPayload::SplashPrepared(MainIdentity {
                    run: expected.run,
                    launch: LaunchId::new(1)?,
                    navigation: Generation::new(1)?
                })
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn unprepared_child_cannot_replace_root_splash_or_manufacture_presented_event() -> Result<()> {
        let (mut child, owner, _) = super::super::test_support::child()?;
        assert!(!child.entry_splash_released());
        assert!(child.entry_splash_frame_presented().is_err());
        assert!(!child.splash.submitted);
        assert!(child.splash.pending.is_none());
        assert!(child.take_startup_event()?.is_none());
        assert!(!child.entry_splash_released());
        drop(child);
        drop(owner);
        Ok(())
    }
}
