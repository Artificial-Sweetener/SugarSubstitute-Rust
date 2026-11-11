// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain host panel messages until the browser admits a panel command slot.

use super::delivery;
use crate::{DesktopError, MainIdentity, PanelCommand};
use sugarsubstitute_contracts::panel::PanelMessage;

/// Supersession retains at most one unsent message per [`Kind`].
const CAPACITY: usize = 2;

/// Host message kinds; a newer message supersedes an unsent one of the same kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    /// Each configuration carries the complete current presentation.
    Configure,
    /// Every heartbeat requests the same connectivity observation.
    Ping,
}

/// One encoded host message qualified for the exact document that admitted it.
pub(in crate::main_actor) struct HostMessage {
    identity: MainIdentity,
    context: i64,
    kind: Kind,
    parameters: String,
    /// Retains the application's admitted allocation until submission or retirement.
    _command: PanelCommand,
}

impl HostMessage {
    /// Encode a host message for the top-document execution context current at admission.
    ///
    /// # Errors
    /// Rejects page-directed message kinds and payloads beyond the native command bound.
    pub(in crate::main_actor) fn new(
        command: PanelCommand,
        context: i64,
    ) -> Result<Self, DesktopError> {
        let kind = match command.message() {
            PanelMessage::Configure { .. } => Kind::Configure,
            PanelMessage::Ping(_) => Kind::Ping,
            _ => {
                return Err(DesktopError::message(
                    "send_panel",
                    "panel message is not host-directed",
                ));
            }
        };
        let payload = command
            .message()
            .encode()
            .map_err(|cause| DesktopError::new("encode_panel_message", cause))?;
        let parameters = delivery::encode(context, &payload)?;
        Ok(Self { identity: command.identity(), context, kind, parameters, _command: command })
    }

    /// Return the execution context this message was encoded for.
    pub(in crate::main_actor) fn context(&self) -> i64 {
        self.context
    }

    /// Borrow the bounded `Runtime.evaluate` parameters for native submission.
    pub(in crate::main_actor) fn parameters(&self) -> &str {
        &self.parameters
    }
}

/// Bounded FIFO of host messages waiting while browser completions hold command slots.
pub(in crate::main_actor) struct HostMessages {
    slots: [Option<HostMessage>; CAPACITY],
    first: usize,
    len: usize,
}

impl HostMessages {
    pub(in crate::main_actor) fn new() -> Self {
        Self { slots: std::array::from_fn(|_| None), first: 0, len: 0 }
    }

    pub(in crate::main_actor) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Supersede an unsent message of the same kind in place; otherwise append in order.
    ///
    /// # Errors
    /// Rejects overload explicitly should a retained kind ever lack its supersession slot.
    pub(super) fn push(&mut self, message: HostMessage) -> Result<(), DesktopError> {
        for offset in 0..self.len {
            let index = (self.first + offset) % CAPACITY;
            if self.slots[index].as_ref().is_some_and(|pending| pending.kind == message.kind) {
                self.slots[index] = Some(message);
                return Ok(());
            }
        }
        if self.len == CAPACITY {
            return Err(DesktopError::message(
                "panel_message_queue",
                "panel message capacity exhausted",
            ));
        }
        self.slots[(self.first + self.len) % CAPACITY] = Some(message);
        self.len += 1;
        Ok(())
    }

    /// Retire messages for a superseded document, then submit the oldest one when admitted.
    ///
    /// `current` is the identity and execution context that may still receive host messages;
    /// it is absent while the window closes or lacks a ready top document. Returns whether a
    /// message was retired or transferred to `send`, whose failure keeps no retained copy.
    pub(super) fn progress(
        &mut self,
        current: Option<(MainIdentity, i64)>,
        admitted: bool,
        send: impl FnOnce(HostMessage) -> Result<(), DesktopError>,
    ) -> Result<bool, DesktopError> {
        let retired = self.retire_superseded(current)?;
        if !admitted || self.is_empty() {
            return Ok(retired);
        }
        send(self.pop()?)?;
        Ok(true)
    }

    /// Release every retained message and its admitted allocation when the window closes.
    pub(in crate::main_actor) fn clear(&mut self) {
        self.slots = std::array::from_fn(|_| None);
        self.first = 0;
        self.len = 0;
    }

    fn retire_superseded(
        &mut self,
        current: Option<(MainIdentity, i64)>,
    ) -> Result<bool, DesktopError> {
        let mut retired = false;
        for _ in 0..self.len {
            let message = self.pop()?;
            if current == Some((message.identity, message.context)) {
                self.slots[(self.first + self.len) % CAPACITY] = Some(message);
                self.len += 1;
            } else {
                tracing::debug!(operation="retire_panel_message", identity=?message.identity,
                    context=message.context, kind=?message.kind,
                    "Retired an unsent host panel message for a superseded document");
                retired = true;
            }
        }
        Ok(retired)
    }

    fn pop(&mut self) -> Result<HostMessage, DesktopError> {
        let message = self.slots[self.first].take().ok_or_else(|| {
            DesktopError::message("panel_message_queue", "retained panel message is missing")
        })?;
        self.first = (self.first + 1) % CAPACITY;
        self.len -= 1;
        Ok(message)
    }
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
