// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Charge known storage before fixed native metadata, staging and duplex allocation.
use super::*;
impl Active {
    pub(super) fn allocate(&mut self, _core: &Core) -> Result<(), EntryTransportError> {
        self.initialized = true;
        let metadata = if self.work.shared.server { 4 * 1024 * 1024 } else { 28 * 1024 };
        let claim = self.context.memory().claim(metadata).map_err(|cause| {
            EntryTransportError::native(NativeObservationError::new(
                "entry_metadata_storage",
                cause,
            ))
        })?;
        *lock(&self.work.shared.metadata) = Some(claim);
        let count = if self.work.shared.server { MAX_ENTRY_CONNECTIONS } else { 1 };
        lock(&self.work.shared.state).allocate(count);
        self.connections = (0..count).map(|_| ConnectionIo::new()).collect();
        let core = self.work.shared.core.upgrade().ok_or(EntryTransportError::Closed)?;
        self.signals = (0..count).map(|_| std::array::from_fn(|_| Signal::new(&core))).collect();
        // Every partial buffer is adopted into Active before the next fallible claim.
        for slot in &mut self.connections {
            slot.write = Some(buffers::Buffer::new(self.context.memory())?);
        }
        if !self.work.shared.server {
            let input = self.work.input.as_ref().ok_or(EntryTransportError::InvalidPhase)?;
            let input_claim =
                self.context.memory().claim(input.request.retained_bytes()).map_err(|cause| {
                    EntryTransportError::native(NativeObservationError::new(
                        "entry_owned_input_storage",
                        cause,
                    ))
                })?;
            let request = EntryRequestId::new(input.source_setup.ticks() as u64)
                .map_err(EntryTransportError::Shape)?;
            let header = protocol::Header {
                namespace: input.facts.namespace(),
                epoch: input.candidate,
                request,
                source: input.source_setup,
                nonce: [0; 32],
            };
            self.client = Some(Box::new(ClientStaging {
                header,
                length: 0,
                hello: [0; protocol::HEADER_BYTES],
            }));
            let client = self.client.as_mut().ok_or(EntryTransportError::InvalidPhase)?;
            client.length = protocol::stage_launch(
                &mut self.connections[0]
                    .write
                    .as_mut()
                    .ok_or(EntryTransportError::InvalidPhase)?
                    .bytes,
                &input.request,
            )?;
            protocol::encode(&mut client.hello, &header, protocol::Payload::Hello)?;
            drop(self.work.input.take());
            drop(input_claim);
        }
        for slot in &mut self.connections {
            slot.read = Some(buffers::Buffer::new(self.context.memory())?);
        }
        if self.work.shared.server {
            self.replay = Some(replay::Replay::new(self.context.memory())?);
        }
        Ok(())
    }
}
