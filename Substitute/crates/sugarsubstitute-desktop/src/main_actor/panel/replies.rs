// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain correlated canvas replies until the browser can accept their delivery.

use crate::{DesktopError, main_native_delivery::NativeDelivery};

const CAPACITY: usize = 16;

/// Bound retained responses without consuming the browser's panel command reserve.
pub(in crate::main_actor) struct Replies {
    slots: [Option<NativeDelivery>; CAPACITY],
    first: usize,
    len: usize,
    reply_after_evidence: bool,
}

impl Replies {
    pub(in crate::main_actor) fn new() -> Self {
        Self { slots: std::array::from_fn(|_| None), first: 0, len: 0, reply_after_evidence: false }
    }

    pub(in crate::main_actor) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Give a retained reply the next live turn after one native evidence item was serviced.
    pub(in crate::main_actor) fn evidence_serviced(&mut self) {
        self.reply_after_evidence = true;
    }

    /// Preserve round-robin custody without granting delivery while the queue is empty.
    pub(in crate::main_actor) fn reply_due(&self) -> bool {
        self.reply_after_evidence && !self.is_empty()
    }

    /// Coalesce unsent full snapshots by attachment and canvas notifications by kind; preserve
    /// correlated responses and order.
    /// Reject independent overload explicitly without acknowledging a discarded request.
    pub(super) fn push(&mut self, reply: NativeDelivery) -> Result<(), DesktopError> {
        if let NativeDelivery::Presentation(incoming) = &reply {
            for offset in 0..self.len {
                let index = (self.first + offset) % CAPACITY;
                if let Some(NativeDelivery::Presentation(pending)) = &self.slots[index]
                    && pending.same_target(incoming)
                {
                    if incoming.sequence() > pending.sequence() {
                        self.slots[index] = Some(reply);
                    }
                    return Ok(());
                }
            }
        }
        if let NativeDelivery::CanvasNotification(incoming) = &reply {
            for offset in 0..self.len {
                let index = (self.first + offset) % CAPACITY;
                if let Some(NativeDelivery::CanvasNotification(pending)) = &self.slots[index]
                    && pending.same_target(incoming)
                {
                    self.slots[index] = Some(reply);
                    return Ok(());
                }
            }
        }
        if let NativeDelivery::Memory(incoming) = &reply {
            for offset in 0..self.len {
                let index = (self.first + offset) % CAPACITY;
                if let Some(NativeDelivery::Memory(pending)) = &self.slots[index] {
                    if incoming.generation() > pending.generation() {
                        self.slots[index] = Some(reply);
                    }
                    return Ok(());
                }
            }
        }
        if self.len == CAPACITY {
            return Err(DesktopError::message("canvas_reply_queue", "reply capacity exhausted"));
        }
        let index = (self.first + self.len) % CAPACITY;
        self.slots[index] = Some(reply);
        self.len += 1;
        Ok(())
    }

    /// Retire obsolete asynchronous claims even while native callback capacity is occupied.
    /// Correlated responses remain in order with their original request credit.
    pub(super) fn retire_presentations(
        &mut self,
        current: Option<crate::MainIdentity>,
    ) -> Result<bool, DesktopError> {
        let mut retired = false;
        for _ in 0..self.len {
            let delivery = self.slots[self.first].take().ok_or_else(|| {
                DesktopError::message("canvas_reply_queue", "retained reply is missing")
            })?;
            self.first = (self.first + 1) % CAPACITY;
            self.len -= 1;
            if matches!(&delivery, NativeDelivery::Presentation(value) if current != Some(value.identity()))
                || matches!(&delivery, NativeDelivery::Memory(value) if current != Some(value.identity()))
                || matches!(&delivery, NativeDelivery::CanvasNotification(value) if current != Some(value.identity()))
            {
                retired = true;
            } else {
                let tail = (self.first + self.len) % CAPACITY;
                self.slots[tail] = Some(delivery);
                self.len += 1;
            }
        }
        Ok(retired)
    }

    /// Transfer ownership to the native command slot; submission failures retain custody there.
    pub(super) fn progress(
        &mut self,
        admitted: bool,
        send: impl FnOnce(NativeDelivery) -> Result<(), DesktopError>,
    ) -> Result<bool, DesktopError> {
        if !admitted || self.is_empty() {
            return Ok(false);
        }
        let reply = self.slots[self.first].take().ok_or_else(|| {
            DesktopError::message("canvas_reply_queue", "retained reply is missing")
        })?;
        self.first = (self.first + 1) % CAPACITY;
        self.len -= 1;
        self.reply_after_evidence = false;
        send(reply)?;
        Ok(true)
    }

    /// Retire undelivered replies when the document closes, releasing their claims.
    pub(in crate::main_actor) fn clear(&mut self) {
        self.slots = std::array::from_fn(|_| None);
        self.first = 0;
        self.len = 0;
        self.reply_after_evidence = false;
    }
}

#[cfg(test)]
#[path = "replies_tests.rs"]
mod tests;
