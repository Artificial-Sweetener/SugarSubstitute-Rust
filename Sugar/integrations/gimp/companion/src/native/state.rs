//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
//! Retain private request correlation and offscreen resource cancellation across renderer reloads.

use super::frames::NativeLimits;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::mpsc::{Receiver, Sender, SyncSender, channel},
};

pub(super) struct Call {
    pub(super) reply: Option<Sender<Result<Value, String>>>,
    pub(super) receiver: Option<Receiver<Result<Value, String>>>,
    pub(super) snapshot: Option<String>,
    pub(super) releasing: Option<String>,
    pub(super) cancelled: bool,
}

pub(super) struct State {
    pub(super) limits: NativeLimits,
    pub(super) writer: Option<SyncSender<Vec<u8>>>,
    pub(super) calls: BTreeMap<u32, Call>,
    pub(super) next: u32,
    pub(super) charged: usize,
    pub(super) closed: bool,
}

impl State {
    /// Cancel and retire one renderer atomically before another renderer can submit work.
    pub(super) fn reset(&mut self) -> Result<(), String> {
        let ids: Vec<u32> = self.calls.keys().copied().collect();
        for id in ids {
            self.cancel(id)?;
        }
        for call in self.calls.values_mut() {
            if let Some(reply) = call.reply.take() {
                let _ = reply.send(Err("The native viewer session was reset.".into()));
            }
        }
        self.calls.clear();
        Ok(())
    }

    /// Admit each mutation identity once and reserve bytes before queueing a frame.
    pub(super) fn submit(&mut self, command: Value) -> Result<u32, String> {
        if self.closed || self.calls.len() >= self.limits.pending_requests {
            return Err("The native GIMP bridge is unavailable or busy.".into());
        }
        if !command.is_object() {
            return Err("A native command object is required.".into());
        }
        let id = self
            .next
            .checked_add(1)
            .filter(|id| *id <= i32::MAX as u32)
            .ok_or("The native request identity space is exhausted.")?;
        self.next = id;
        let releasing = if command.get("type").and_then(Value::as_str) == Some("snapshot.release") {
            command
                .get("token")
                .and_then(Value::as_str)
                .map(str::to_owned)
        } else {
            None
        };
        let (reply, receiver) = channel();
        self.enqueue(&json!({"type":"request","id":id,"command":command}))?;
        self.calls.insert(
            id,
            Call {
                reply: Some(reply),
                receiver: Some(receiver),
                snapshot: None,
                releasing,
                cancelled: false,
            },
        );
        Ok(id)
    }

    /// Retain a completed snapshot until its explicit release or cancellation reaches Python.
    pub(super) fn receive(&mut self, id: u32, result: Value) {
        let releasing = self.calls.get(&id).and_then(|call| {
            (result.get("kind").and_then(Value::as_str) == Some("released"))
                .then(|| call.releasing.clone())
                .flatten()
        });
        if let Some(token) = releasing {
            self.calls
                .retain(|_, call| call.snapshot.as_deref() != Some(&token));
        }
        if let Some(call) = self.calls.get_mut(&id) {
            if result.get("kind").and_then(Value::as_str) == Some("snapshot") {
                call.snapshot = result
                    .get("token")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            if let Some(reply) = call.reply.take() {
                // A disposed renderer may abandon its wait; reset still owns snapshot cancellation.
                let _ = reply.send(Ok(result));
            }
        }
    }

    /// Queue cancellation once without deleting a successfully committed user layer.
    pub(super) fn cancel(&mut self, id: u32) -> Result<(), String> {
        let Some(call) = self.calls.get(&id) else {
            return Ok(());
        };
        if call.cancelled {
            return Ok(());
        }
        let completed_snapshot =
            call.snapshot.is_some() && call.receiver.is_none() && call.reply.is_none();
        self.enqueue(&json!({"type":"cancel","id":id}))?;
        if completed_snapshot {
            self.calls.remove(&id);
        } else if let Some(call) = self.calls.get_mut(&id) {
            call.cancelled = true;
        }
        Ok(())
    }

    /// Refuse queued frames by both count and bytes before an IO worker can block.
    fn enqueue(&mut self, message: &Value) -> Result<(), String> {
        let mut frame = serde_json::to_vec(message)
            .map_err(|error| format!("Encode native request: {error}"))?;
        frame.push(b'\n');
        let size = frame.len();
        if self.closed || size > self.limits.frame_bytes.saturating_sub(self.charged) {
            return Err("The native GIMP pipe is unavailable or busy.".into());
        }
        self.writer
            .as_ref()
            .ok_or("The native writer has closed.")?
            .try_send(frame)
            .map_err(|_| "The native writer is unavailable or busy.")?;
        self.charged += size;
        Ok(())
    }
}
