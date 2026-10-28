// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain correlated canvas replies until the browser can accept their delivery.

use crate::{CanvasContextReply, DesktopError, MainIdentity, SurfaceReply};

const CAPACITY: usize = 16;

/// Keep each encoded response with its original navigation and allocation claim.
pub(super) enum Reply {
    Context(CanvasContextReply),
    Surface(SurfaceReply),
}

impl Reply {
    fn identity(&self) -> MainIdentity {
        match self {
            Self::Context(reply) => reply.identity(),
            Self::Surface(reply) => reply.identity(),
        }
    }

    fn payload(&self) -> &[u8] {
        match self {
            Self::Context(reply) => reply.payload(),
            Self::Surface(reply) => reply.payload(),
        }
    }
}

/// Bound retained responses without consuming the browser's panel command reserve.
pub(in crate::main_actor) struct Replies {
    slots: [Option<Reply>; CAPACITY],
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

    /// Fail explicitly on overload rather than acknowledging a discarded request.
    pub(super) fn push(&mut self, reply: Reply) -> Result<(), DesktopError> {
        if self.len == CAPACITY {
            return Err(DesktopError::message("canvas_reply_queue", "reply capacity exhausted"));
        }
        let index = (self.first + self.len) % CAPACITY;
        self.slots[index] = Some(reply);
        self.len += 1;
        Ok(())
    }

    /// Transfer only after the native submission succeeds; busy and failed attempts retain custody.
    pub(super) fn progress(
        &mut self,
        admitted: bool,
        send: impl FnOnce(MainIdentity, &[u8]) -> Result<(), DesktopError>,
    ) -> Result<bool, DesktopError> {
        if !admitted || self.is_empty() {
            return Ok(false);
        }
        let reply = self.slots[self.first].as_ref().ok_or_else(|| {
            DesktopError::message("canvas_reply_queue", "retained reply is missing")
        })?;
        send(reply.identity(), reply.payload())?;
        self.slots[self.first] = None;
        self.first = (self.first + 1) % CAPACITY;
        self.len -= 1;
        self.reply_after_evidence = false;
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
