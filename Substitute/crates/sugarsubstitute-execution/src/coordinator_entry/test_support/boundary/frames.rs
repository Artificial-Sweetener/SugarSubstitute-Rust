// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer each frame the owner writes as the scripted peer on the other end of the pipe would.
use super::*;
impl Boundary {
    /// Record one written frame and queue the scripted peer's answer for the next read.
    pub(super) fn answer(
        &self,
        index: usize,
        bytes: &[u8],
    ) -> Result<Progress, EntryTransportError> {
        self.writes.fetch_add(1, Ordering::AcqRel);
        let frame = protocol::decode(bytes)?;
        if matches!(
            frame.payload,
            protocol::DecodedPayload::Accepted(_) | protocol::DecodedPayload::Rejected(_)
        ) && let Some(error) = lock(&self.outcome_write_failure).take()
        {
            return Err(error);
        }
        let final_reply = matches!(
            frame.payload,
            protocol::DecodedPayload::Accepted(_)
                | protocol::DecodedPayload::Rejected(_)
                | protocol::DecodedPayload::Unresponsive
                | protocol::DecodedPayload::Closing
                | protocol::DecodedPayload::Initializing
        );
        match frame.payload {
            protocol::DecodedPayload::Challenge if self.server => {
                let mut header = frame.header;
                if self.changed_nonce.load(Ordering::Acquire) {
                    header.nonce[0] ^= 1;
                }
                let request =
                    request().map_err(|cause| failure_from("entry_fixture_request", cause))?;
                self.push(index, header, protocol::Payload::Launch(&request))?;
            }
            protocol::DecodedPayload::Hello if !self.server => {
                let mut header = frame.header;
                if self
                    .initializing
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| count.checked_sub(1))
                    .is_ok()
                {
                    header.epoch = None;
                    self.push(index, header, protocol::Payload::Initializing)?;
                } else {
                    header.epoch = Some(
                        owner().map_err(|cause| failure_from("entry_fixture_owner", cause))?.epoch,
                    );
                    let nonce = self.nonce.fetch_add(1, Ordering::AcqRel) + 1;
                    header.nonce =
                        [u8::try_from(nonce).map_err(|_| EntryTransportError::Capacity)?; 32];
                    let payload = if self.closing_before_launch.load(Ordering::Acquire) {
                        protocol::Payload::Closing
                    } else {
                        protocol::Payload::Challenge
                    };
                    self.push(index, header, payload)?;
                }
            }
            protocol::DecodedPayload::Launch(_) if !self.server => {
                lock(&self.client_launches).push(frame.header);
                if self.disconnect_after_launch.load(Ordering::Acquire) {
                    *lock(&self.read_failure) =
                        Some(EntryTransportError::native(NativeObservationError::new(
                            "pipe_completion",
                            windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(
                                109,
                            )),
                        )));
                } else if self.closing_after_launch.load(Ordering::Acquire) {
                    self.push(index, frame.header, protocol::Payload::Closing)?;
                } else if self.unresponsive_after_launch.load(Ordering::Acquire) {
                    self.push(index, frame.header, protocol::Payload::Unresponsive)?;
                } else if !self.defer_client_outcome.load(Ordering::Acquire) {
                    self.push(
                        index,
                        frame.header,
                        protocol::Payload::Accepted(
                            run().map_err(|cause| failure_from("entry_fixture_run", cause))?,
                        ),
                    )?;
                }
            }
            protocol::DecodedPayload::Accepted(run) => {
                lock(&self.replies).push((frame.header, EntryOutcome::Accepted(run)))
            }
            protocol::DecodedPayload::Rejected(reason) => {
                lock(&self.replies).push((frame.header, EntryOutcome::Rejected(reason)))
            }
            protocol::DecodedPayload::Unresponsive if self.server => {
                lock(&self.replies).push((frame.header, EntryOutcome::Unresponsive))
            }
            protocol::DecodedPayload::Closing if self.server => {
                lock(&self.replies).push((frame.header, EntryOutcome::Closing))
            }
            protocol::DecodedPayload::Initializing => {}
            _ => return Err(EntryTransportError::InvalidPhase),
        }
        if self.server && !self.hold_peer_close.load(Ordering::Acquire) && final_reply {
            lock(&self.peer_closed).insert(index);
        }
        Ok(Progress::Complete(bytes.len() as u32))
    }
}
