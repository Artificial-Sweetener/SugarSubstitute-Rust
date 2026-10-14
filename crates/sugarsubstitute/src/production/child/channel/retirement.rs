// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence only original acquisition/output requests while keeping native custody explicit.
use super::super::requests::Purpose;
use super::*;
impl ChildChannel {
    /// Designate the original output owner before its actual native write.
    pub(in super::super) fn output_pull(&mut self, header: WireHeader) -> Result<()> {
        self.requests.as_mut().ok_or("Original ledger absent")?.output_pull(header)?;
        Ok(())
    }
    /// Read actual queue admission without withdrawing or changing its native custody.
    pub(in super::super) fn request_prepared(&self, header: WireHeader) -> Result<bool> {
        Ok(self.requests.as_ref().ok_or("Original ledger absent")?.prepared(header)?)
    }
    /// Close this exact finite owner, preserving the slot/header/cutoff and actual write ticket.
    pub(in super::super) fn retire_request(
        &mut self,
        header: WireHeader,
        purpose: Purpose,
    ) -> Result<bool> {
        let first =
            self.requests.as_mut().ok_or("Original ledger absent")?.retire(header, purpose)?;
        if let Some(write) = &mut self.write
            && write.header() == header
        {
            write.request_retirement();
        }
        if self.received_header()? == Some(header) {
            let binding =
                self.binding().ok_or("Original authenticated retirement channel closed")?;
            let decoded = FrameCodec::decode(
                self.received.as_ref().ok_or("Actual retained original frame absent")?.as_bytes(),
            )?;
            if wire::incoming(binding, &decoded)? != wire::IncomingKind::Reply {
                return Err("Retiring original frame is not a reply".into());
            }
            self.requests
                .as_mut()
                .ok_or("Original ledger absent")?
                .retain_retiring_receipt(header)?;
        }
        Ok(first)
    }
    /// Return validated nonfinal acquisition correlation without accepting its authority.
    pub(in super::super) fn disposed_progress(&mut self, header: WireHeader) -> Result<()> {
        self.requests.as_mut().ok_or("Original ledger absent")?.disposed_progress(header)?;
        Ok(())
    }
    /// Original native writes must actually complete or return their checked vetoed buffer.
    pub(in super::super) fn writes_pending(&self) -> bool {
        self.write.as_ref().is_some_and(|write| !write.returned())
    }
    /// Borrow only this exact correlation's actual retirement state.
    pub(in super::super) fn retiring_request(&self, header: WireHeader) -> Result<bool> {
        Ok(self.requests.as_ref().ok_or("Original ledger absent")?.retiring(header)?)
    }
    /// Transfer actual original no-issue native custody together with its checked ledger withdrawal.
    /// A phase marker without the exact returned buffer/ticket cannot produce this local proof.
    pub(in super::super) fn withdraw_acquisition(
        &mut self,
        header: WireHeader,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
    ) -> Result<(super::super::requests::WithdrawnRequest, Option<write::UnissuedAcquisition>)>
    {
        let unissued = self.never_issued(header)?;
        if unissued {
            let owner = self
                .write
                .as_mut()
                .filter(|owner| owner.header() == header)
                .ok_or("Original no-issue native owner absent")?;
            if owner.returned_acquisition()? != Some(identity) {
                return Err("Original returned acquisition identity changed".into());
            }
        }
        let proof =
            self.requests.as_mut().ok_or("Original ledger absent")?.withdraw_acquisition(header)?;
        let native = if unissued {
            Some(
                self.write
                    .take()
                    .ok_or("Original returned native owner disappeared")?
                    .into_unissued(identity),
            )
        } else {
            None
        };
        Ok((proof, native))
    }
    /// Borrow an observed no-issue veto, not a declaration of native or role settlement.
    pub(in super::super) fn never_issued(&self, header: WireHeader) -> Result<bool> {
        Ok(self.requests.as_ref().ok_or("Original ledger absent")?.never_issued(header)?)
    }
    /// Retire original semantic correlations only after the authenticated exact full ACK.
    /// No original native write or unrelated received loan may remain at this boundary.
    pub(in super::super) fn settle_requests(
        &mut self,
        ack: WireHeader,
        frame: &ReceivedLifecycleFrame,
        acquisition: Option<WireHeader>,
        output: Option<WireHeader>,
    ) -> Result<()> {
        if self.write.is_some() || self.received.is_some() {
            return Err("Native write or unrelated original frame remains".into());
        }
        let binding = self.binding().ok_or("Original authenticated channel closed")?;
        let decoded = FrameCodec::decode(frame.as_bytes())?;
        if self.peer().is_none_or(|peer| peer.run() != ack.run)
            || binding.run() != ack.run
            || binding.connection() != ack.connection
            || decoded.header != ack
            || !matches!(
                decoded.payload,
                sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload::Reply(
                    sugarsubstitute_supervision::lifecycle_protocol::ReplyOutcome::Acknowledged
                )
            )
        {
            return Err("Original full attempt settlement ACK differs".into());
        }
        let ledger = self.requests.as_mut().ok_or("Original ledger absent")?;
        if let Some(header) = acquisition {
            ledger.can_settle(header, Purpose::Acquisition)?;
        }
        if let Some(header) = output {
            ledger.can_settle(header, Purpose::OutputPull)?;
        }
        ledger.consumed(ack)?;
        if let Some(header) = acquisition {
            ledger.settled(header, Purpose::Acquisition)?;
        }
        if let Some(header) = output {
            ledger.settled(header, Purpose::OutputPull)?;
        }
        Ok(())
    }
}
