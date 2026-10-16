// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind real protocol correlation for output tests, leaving the native transport absent.
use super::*;
use sugarsubstitute_supervision::lifecycle_protocol::ReplyOutcome;
impl ChildChannel {
    /// Install only a completed wire handshake and real request ledger; this creates no native proof.
    pub(in crate::production::child) fn output_polling_fixture(
        &mut self,
        client: &ExecutionClient,
        connection: sugarsubstitute_contracts::lifecycle::ConnectionId,
    ) -> Result<()> {
        let encoded = client.encode_counter_deadline(
            client.now()?.checked_add(std::time::Duration::from_secs(30))?,
        )?;
        let original = WireHeader {
            run: self.expected_run,
            connection: Some(connection),
            request: None,
            deadline_qpc: encoded.ticks(),
        };
        let mut handshake = WireHandshake::new(self.expected_run, encoded);
        handshake.hello_admitted()?;
        let mut bytes = [0; 256];
        let count = FrameCodec::encode(
            &WireMessage {
                header: original,
                payload: WirePayload::Welcome { qpc_frequency: encoded.frequency() },
            },
            &mut bytes,
        )?;
        handshake.welcome(&FrameCodec::decode(&bytes[..count])?)?;
        let request = RequestId::new(1)?;
        handshake.register_admitted(request)?;
        let count = FrameCodec::encode(
            &WireMessage {
                header: WireHeader { request: Some(request), ..original },
                payload: WirePayload::Reply(ReplyOutcome::Acknowledged),
            },
            &mut bytes,
        )?;
        handshake.registered(&FrameCodec::decode(&bytes[..count])?)?;
        self.requests = Some(ClientRequests::new(&handshake)?);
        self.handshake = Some(handshake);
        self.client = Some(client.clone());
        Ok(())
    }
}
