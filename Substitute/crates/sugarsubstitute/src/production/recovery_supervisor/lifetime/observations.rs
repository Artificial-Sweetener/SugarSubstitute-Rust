// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume genuine protocol observations during startup and original retirement alike.
use super::*;
use sugarsubstitute_contracts::recovery_supervisor::wire::{
    RecoveryCommand, RecoveryReceiver, decode,
};
impl Lifetime {
    pub(super) fn observe_protocol(&mut self) -> Result<()> {
        if self.channel.is_none() {
            return Ok(());
        }
        let client = self.client.as_ref().ok_or("Recovery execution absent")?;
        let finite = self.finite.as_ref().ok_or("Recovery finite scope absent")?;
        let native = self.native.as_ref().ok_or("Recovery native scope absent")?;
        if !self.closing
            && !matches!(self.stage, Stage::Running | Stage::ExitedQueued | Stage::Finished)
            && client.now()?
                >= self.channel.as_ref().ok_or("Recovery channel absent")?.original_until()
        {
            return Err("Original recovery setup deadline expired".into());
        }
        if let Some(sent) = self.channel.as_mut().and_then(Channel::take_sent) {
            self.stage = match (sent, self.stage) {
                (Sent::Hello, stage) => stage,
                (Sent::StartupAccepted, Stage::AcceptedQueued) => {
                    if self.closing {
                        Stage::AcceptedQueued
                    } else {
                        let capture = self.capture.as_mut().ok_or("Fresh capture owner absent")?;
                        capture.start(client, native, finite, &self.jobs)?;
                        Stage::Registering
                    }
                }
                (Sent::CaptureRegistered, Stage::RegisteredQueued) => {
                    if self.closing {
                        Stage::RegisteredQueued
                    } else {
                        self.capture
                            .as_mut()
                            .ok_or("Fresh capture owner absent")?
                            .attach(client, native, &self.jobs)?;
                        Stage::Attaching
                    }
                }
                (Sent::DesktopCreated, Stage::CreatedQueued) => Stage::Enrollment,
                (Sent::DesktopExited, Stage::ExitedQueued) => Stage::Finished,
                (Sent::Retired, stage) if self.closing => {
                    self.notice.written()?;
                    stage
                }
                _ => return Err("Recovery actual write phase rejected".into()),
            };
        }
        let channel = self.channel.as_mut().ok_or("Recovery channel absent")?;
        if self.origin.is_some()
            && let Some(frame) = channel.control_frame()
        {
            match decode(frame.as_bytes(), RecoveryReceiver::Supervisor)?.command {
                RecoveryCommand::Stop(counter) => {
                    channel.control().validate_current_admission()?;
                    if !self.notice.stop_received() {
                        let until = client.decode_counter_deadline(counter)?;
                        self.cleanup.root_stop(client.now()?, until)?;
                    }
                    self.notice.accept_stop(counter)?;
                    // Keep this actual receive loan until checked Retired Written and close.
                    // It forbids another read and prevents terminal from overtaking that write.
                }
                RecoveryCommand::Failed(code) => {
                    return Err(format!("Actual Root stopped recovery: {code:?}").into());
                }
                RecoveryCommand::DesktopEnrolled(_) => {}
                _ => return Err("Recovery post-startup Root command rejected".into()),
            }
        }
        Ok(())
    }
}
