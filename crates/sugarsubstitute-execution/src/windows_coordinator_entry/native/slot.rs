// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one entry pipe lifetime and all partial I/O/credential ownership through checked retirement.
use super::*;
pub(super) struct Slot {
    pub(super) pipe: Handle,
    pub(super) peer: PeerPin,
    pub(super) operations: [Option<Operation>; 3],
    pub(super) signals: Option<[Arc<Signal>; 3]>,
    pub(super) read_completed: bool,
    pub(super) connected: bool,
}
impl Slot {
    pub(super) fn new() -> Self {
        Self {
            pipe: Handle(0),
            peer: PeerPin::new(),
            operations: std::array::from_fn(|_| None),
            signals: None,
            read_completed: false,
            connected: false,
        }
    }
    pub(super) fn operation(&mut self, role: Role) -> Result<&mut Operation, EntryTransportError> {
        self.operations[role.index()].as_mut().ok_or(EntryTransportError::InvalidPhase)
    }
    pub(super) fn retire(
        &mut self,
        reuse: bool,
        server: bool,
    ) -> Result<bool, EntryTransportError> {
        let mut settled = true;
        let mut first = None;
        for operation in self.operations.iter_mut().flatten() {
            match operation.retire(self.pipe.raw()) {
                Ok(retired) => settled &= retired,
                Err(error) => {
                    settled = false;
                    security::retain(&mut first, error.into());
                }
            }
        }
        if let Some(error) = first {
            return Err(error);
        }
        if !settled {
            return Ok(false);
        }
        self.peer.retire()?;
        if reuse && server {
            if self.connected {
                // SAFETY: all I/O settled and callbacks quiesced before disconnecting this owned pipe.
                unsafe { DisconnectNamedPipe(self.pipe.raw()) }
                    .map_err(|cause| error("entry_disconnect", cause))?;
            }
            let signals = self.signals.as_ref().ok_or(EntryTransportError::InvalidPhase)?;
            self.operations =
                std::array::from_fn(|role| Some(Operation::new(signals[role].clone())));
        } else {
            self.pipe.close("entry_close_pipe")?;
            self.operations = if reuse {
                let signals = self.signals.as_ref().ok_or(EntryTransportError::InvalidPhase)?;
                std::array::from_fn(|role| Some(Operation::new(signals[role].clone())))
            } else {
                std::array::from_fn(|_| None)
            };
        }
        self.read_completed = false;
        self.connected = false;
        Ok(true)
    }
}
