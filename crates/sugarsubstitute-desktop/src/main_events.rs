// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preclaim and reuse bounded native callback storage until receiver retirement.

use crate::{DesktopError, main_readiness_types::ProbeTarget};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

/// Limit authored event payloads by actual UTF-8 allocation bytes.
pub(crate) const PAYLOAD_BYTES: usize = 32768;
/// Bound the native UTF-16 scan independently of decoded UTF-8 bytes, including NUL.
pub(crate) const NATIVE_UNITS: usize = 16384;
const SLOTS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Runtime,
    Page,
    Probe(ProbeTarget),
    Barrier(ProbeTarget),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Frame,
    FrameDetached,
    Context,
    ContextDestroyed,
    ContextsCleared,
    Console,
    Exception,
    NavigationStarting,
    Navigation { succeeded: bool },
    ProcessFailed { kind: i32 },
    Command,
}

struct Slot {
    kind: Option<Kind>,
    command: Option<Command>,
    payload: String,
}

/// A queue entry is borrowed only during a normal bounded event-loop drain.
pub(crate) struct Observation<'a> {
    pub(crate) kind: Kind,
    pub(crate) command: Option<Command>,
    pub(crate) payload: &'a str,
}

struct Storage {
    slots: Vec<Slot>,
    first: usize,
    count: usize,
    failure: Option<DesktopError>,
    _claim: MemoryClaim,
}

/// Own callback storage on the UI thread; callbacks notify without running host decisions.
///
/// Native getters must return before borrowing this queue. This prevents a native getter
/// that pumps callbacks from reentering a live RefCell borrow. Payload copying below is
/// authored bounded work and performs no native calls. Receivers and callback invocations
/// must retire before the last owner/claim is released.
pub(crate) struct Events {
    storage: RefCell<Storage>,
    accepting: Cell<bool>,
    callback_depth: Cell<usize>,
    wake: Box<dyn Fn()>,
}

impl Events {
    pub(crate) fn new(
        lease: &MemoryLease,
        wake: impl Fn() + 'static,
    ) -> Result<Rc<Self>, DesktopError> {
        let bytes = (SLOTS * (PAYLOAD_BYTES + size_of::<Slot>())
            + size_of::<Self>()
            + size_of::<Storage>()) as u64;
        let claim =
            lease.claim(bytes).map_err(|cause| DesktopError::new("claim_main_events", cause))?;
        let mut storage =
            Storage { slots: Vec::new(), first: 0, count: 0, failure: None, _claim: claim };
        storage
            .slots
            .try_reserve_exact(SLOTS)
            .map_err(|cause| DesktopError::new("allocate_main_event_slots", cause))?;
        if storage.slots.capacity() != SLOTS {
            return Err(DesktopError::message(
                "allocate_main_event_slots",
                "event capacity exceeded",
            ));
        }
        for _ in 0..SLOTS {
            let mut payload = String::new();
            payload
                .try_reserve_exact(PAYLOAD_BYTES)
                .map_err(|cause| DesktopError::new("allocate_main_event_payload", cause))?;
            if payload.capacity() != PAYLOAD_BYTES {
                return Err(DesktopError::message(
                    "allocate_main_event_payload",
                    "payload capacity exceeded",
                ));
            }
            storage.slots.push(Slot { kind: None, command: None, payload });
        }
        Ok(Rc::new(Self {
            storage: RefCell::new(storage),
            accepting: Cell::new(true),
            callback_depth: Cell::new(0),
            wake: Box::new(wake),
        }))
    }

    /// Count actual callback entry/return separately from queued observations.
    pub(crate) fn callback(&self) -> Callback<'_> {
        self.callback_depth.set(self.callback_depth.get() + 1);
        Callback(self)
    }

    pub(crate) fn in_callback(&self) -> bool {
        self.callback_depth.get() != 0
    }

    /// Stop evidence admission before removing native subscriptions.
    pub(crate) fn close_admission(&self) {
        self.accepting.set(false);
    }

    /// Record a getter cause before any cleanup or conversion; always preserve the first.
    pub(crate) fn fail(&self, cause: DesktopError) {
        DesktopError::retain_first(&mut self.storage.borrow_mut().failure, cause);
        self.accepting.set(false);
        (self.wake)();
    }

    pub(crate) fn take_failure(&self) -> Option<DesktopError> {
        self.storage.borrow_mut().failure.take()
    }

    /// Copy only after a native getter has returned and its ownership guard is installed.
    /// Overflow and malformed UTF-16 close observation admission permanently.
    pub(crate) fn record(
        &self,
        kind: Kind,
        copy: impl FnOnce(&mut String) -> Result<(), DesktopError>,
    ) {
        self.record_with(kind, None, copy);
    }
    /// Copy a completion's fixed metadata into preclaimed slots without allocating in callbacks.
    pub(crate) fn record_command(
        &self,
        command: Command,
        copy: impl FnOnce(&mut String) -> Result<(), DesktopError>,
    ) {
        self.record_with(Kind::Command, Some(command), copy);
    }
    fn record_with(
        &self,
        kind: Kind,
        command: Option<Command>,
        copy: impl FnOnce(&mut String) -> Result<(), DesktopError>,
    ) {
        if !self.accepting.get() {
            return;
        }
        let result = (|| {
            let mut storage = self.storage.borrow_mut();
            if storage.count == SLOTS {
                return Err(DesktopError::message(
                    "main_event_overflow",
                    "bounded receiver queue exhausted",
                ));
            }
            let index = (storage.first + storage.count) % SLOTS;
            let slot = &mut storage.slots[index];
            slot.payload.clear();
            copy(&mut slot.payload)?;
            if slot.payload.len() > PAYLOAD_BYTES || slot.payload.capacity() != PAYLOAD_BYTES {
                return Err(DesktopError::message(
                    "main_event_payload",
                    "bounded receiver payload exceeded",
                ));
            }
            slot.kind = Some(kind);
            slot.command = command;
            storage.count += 1;
            Ok(())
        })();
        if let Err(cause) = result {
            self.fail(cause);
        } else {
            (self.wake)();
        }
    }

    /// Consume one entry without cloning/copying it; never invoke native effects in consume.
    /// Failed consumers still dispose this entry while the host retains its original failure.
    pub(crate) fn consume(
        &self,
        consume: impl FnOnce(Observation<'_>) -> Result<(), DesktopError>,
    ) -> Result<bool, DesktopError> {
        if self.in_callback() {
            return Err(DesktopError::message("drain_main_events", "native callback cannot drain"));
        }
        let mut storage = self.storage.borrow_mut();
        if storage.count == 0 {
            return Ok(false);
        }
        let index = storage.first;
        let slot = &mut storage.slots[index];
        let kind = slot
            .kind
            .take()
            .ok_or_else(|| DesktopError::message("drain_main_events", "missing event identity"))?;
        let result =
            consume(Observation { kind, command: slot.command.take(), payload: &slot.payload });
        slot.payload.clear();
        storage.first = (storage.first + 1) % SLOTS;
        storage.count -= 1;
        result.map(|()| true)
    }

    /// Preserve bounded remaining work across event-loop drains without fabricating completion.
    pub(crate) fn remaining(&self) -> usize {
        self.storage.borrow().count
    }
    pub(crate) fn rewake(&self) {
        if self.remaining() != 0 {
            (self.wake)();
        }
    }
}

pub(crate) struct Callback<'a>(&'a Events);
impl Drop for Callback<'_> {
    fn drop(&mut self) {
        self.0.callback_depth.set(self.0.callback_depth.get() - 1);
    }
}

/// Copy exact valid UTF-16 into already admitted UTF-8 storage without allocation growth.
/// The native reader is a pointer read of already returned COM data, never a native getter.
pub(crate) fn copy_utf16(
    output: &mut String,
    mut read: impl FnMut(usize) -> u16,
) -> Result<(), DesktopError> {
    let mut length = None;
    for index in 0..NATIVE_UNITS {
        if read(index) == 0 {
            length = Some(index);
            break;
        }
    }
    let length = length.ok_or_else(|| {
        DesktopError::message("copy_main_observation", "native payload exceeds UTF-16 bound")
    })?;
    let mut bytes = 0;
    for decoded in char::decode_utf16((0..length).map(&mut read)) {
        let character =
            decoded.map_err(|cause| DesktopError::new("copy_main_observation", cause))?;
        bytes += character.len_utf8();
        if bytes > PAYLOAD_BYTES {
            return Err(DesktopError::message(
                "copy_main_observation",
                "decoded payload exceeds UTF-8 bound",
            ));
        }
    }
    if output.capacity() < bytes {
        return Err(DesktopError::message(
            "copy_main_observation",
            "payload storage was not admitted",
        ));
    }
    for decoded in char::decode_utf16((0..length).map(read)) {
        output.push(decoded.map_err(|cause| DesktopError::new("copy_main_observation", cause))?);
    }
    Ok(())
}

#[cfg(test)]
#[path = "main_events_tests.rs"]
mod tests;
