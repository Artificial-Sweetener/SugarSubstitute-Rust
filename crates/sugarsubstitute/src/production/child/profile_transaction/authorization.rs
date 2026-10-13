// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Convert only the original authenticated channel reply into one native desktop command.
use super::*;
use state::Action;
use sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload;
impl ProfileTransaction {
    /// Acquire the actual channel-held frame, validate it without mutation, then consume the
    /// corresponding received request before authorizing one native stage. No other transport
    /// frame or copied reply/header can be supplied to this authorization boundary.
    /// # Errors
    /// Returns the identical original read guard on all failures; no native command is issued.
    pub(in super::super) fn take_reply(
        &mut self,
        channel: &mut ChildChannel,
    ) -> Result<Option<DesktopStartupCommand>, ProfileReplyFailure> {
        if !self.awaiting_reply() {
            return Ok(None);
        }
        let Some(frame) = channel.take_frame() else { return Ok(None) };
        match self.accept(channel, &frame) {
            Ok(command) => {
                drop(frame);
                Ok(command)
            }
            Err(error) => Err(ProfileReplyFailure { frame, error }),
        }
    }
    fn accept(
        &mut self,
        channel: &mut ChildChannel,
        frame: &ReceivedLifecycleFrame,
    ) -> Result<Option<DesktopStartupCommand>, Box<dyn std::error::Error>> {
        self.check()?;
        let binding = channel.binding().ok_or(ProfileError::ChannelClosed)?;
        let decoded = FrameCodec::decode(frame.as_bytes())?;
        if channel.peer().is_none_or(|peer| peer.run() != self.identity().run)
            || binding.run() != self.identity().run
            || decoded.header.run != binding.run()
            || decoded.header.connection != binding.connection()
        {
            return Err(ProfileError::Identity.into());
        }
        let DecodedPayload::Reply(reply) = decoded.payload else {
            return Err(ProfileError::Reply.into());
        };
        let action = self.progress.reply(
            self.pending.ok_or(ProfileError::Order)?,
            decoded.header,
            reply,
            self.browser,
        )?;
        if action == Action::PrepareMain && self.request.is_none() {
            return Err(ProfileError::Order.into());
        }
        self.check()?;
        channel.consume_reply(decoded.header)?;
        self.pending = None;
        self.progress.apply(action);
        match action {
            Action::Advance(_) => Ok(None),
            Action::PrepareMain => self
                .request
                .take()
                .map(DesktopStartupCommand::PrepareMain)
                .map(Some)
                .ok_or_else(|| ProfileError::Order.into()),
            Action::AuthorizeEnvironment => {
                Ok(Some(DesktopStartupCommand::AuthorizeEnvironment(self.identity())))
            }
            Action::AuthorizeBrowser => {
                Ok(Some(DesktopStartupCommand::AuthorizeBrowser(self.identity())))
            }
        }
    }
}
