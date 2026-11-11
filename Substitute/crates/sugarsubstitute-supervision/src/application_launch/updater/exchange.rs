// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The stable launcher is the sole application creator for one authenticated engine exchange.
use super::super::broker::{LaunchReceiver, LaunchResult};
use super::*;
use sugarsubstitute_execution::activation_channel::{ChannelProgress, PreparedChannel};

/// Finite worker phases keep native outputs and creation facts outside result delivery.
enum Phase {
    Receive,
    Receiving,
    Create,
    Finish,
    Send,
    Sending,
    Replied,
}

/// Parent-owned transport and creator. Engine containment never includes the created application.
pub(super) struct Exchange {
    pub(super) channel: PreparedChannel,
    receiver: LaunchReceiver,
    phase: Phase,
    result: Option<LaunchResult>,
    channel_retired: bool,
}
impl Exchange {
    /// Adopt the connected channel before process command composition can fail.
    pub(super) fn new(channel: PreparedChannel) -> Result<Self, ClaimError> {
        Ok(Self {
            receiver: LaunchReceiver::new(channel.identity())?,
            channel,
            phase: Phase::Receive,
            result: None,
            channel_retired: false,
        })
    }
    /// Advance one phase without waiting on peer activity. The exact retained engine process
    /// supplies native duplicate authority; successful exit status never substitutes for creation.
    pub(super) fn advance(
        &mut self,
        process: HANDLE,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        let endpoint = self.channel.endpoint();
        match self.phase {
            Phase::Receive => {
                self.phase = Phase::Receiving;
                endpoint.receive(context).map_err(channel_error)?;
            }
            Phase::Receiving => match endpoint.poll(context).map_err(channel_error)? {
                ChannelProgress::Pending => {}
                ChannelProgress::Received(frame) => {
                    self.receiver.capture_from(process, frame.bytes(), context)?;
                    self.phase = Phase::Create;
                }
                ChannelProgress::Sent => {
                    return Err(ClaimError::invalid("activation_request_direction"));
                }
            },
            Phase::Create => {
                self.phase = Phase::Finish;
                if let Err(cause) = self.receiver.launch(context) {
                    tracing::warn!(operation="activation_application_creation", cause=?cause, "Reporting actual native creation disposition to the owning engine");
                }
            }
            Phase::Finish => {
                self.result = Some(self.receiver.finish(context)?);
                self.phase = Phase::Send;
            }
            Phase::Send => {
                self.phase = Phase::Sending;
                endpoint
                    .send(
                        self.result
                            .as_ref()
                            .ok_or_else(|| ClaimError::invalid("activation_reply_absent"))?
                            .frame(),
                        context,
                    )
                    .map_err(channel_error)?;
            }
            Phase::Sending => match endpoint.poll(context).map_err(channel_error)? {
                ChannelProgress::Pending => {}
                ChannelProgress::Sent => self.phase = Phase::Replied,
                ChannelProgress::Received(_) => {
                    return Err(ClaimError::invalid("activation_reply_direction"));
                }
            },
            Phase::Replied => {}
        }
        Ok(())
    }
    /// Close creator admission and retire all duplicated objects and native channel operations.
    /// Actual creation remains retained even when sending the engine's reply failed.
    pub(super) fn retire(&mut self, context: &JobContext) -> Result<bool, ClaimError> {
        if self.result.is_none() {
            self.result = Some(self.receiver.finish(context)?);
        }
        if !self.channel_retired {
            self.channel.retire_child().map_err(channel_error)?;
            self.channel_retired =
                self.channel.endpoint().retire(context).map_err(channel_error)?;
        }
        Ok(self.channel_retired)
    }
    /// Require actual retired creator status before fallback or successful launcher completion.
    pub(super) fn outcome(&self) -> Result<CreationOutcome, ClaimError> {
        self.result
            .as_ref()
            .map(LaunchResult::outcome)
            .ok_or_else(|| ClaimError::invalid("activation_creation_unobserved"))
    }
}

/// Preserve the execution adapter's native error chain across supervision's public boundary.
pub(super) fn channel_error(
    cause: sugarsubstitute_execution::activation_channel::NativeObservationError,
) -> ClaimError {
    ClaimError::new("activation_channel", cause)
}
