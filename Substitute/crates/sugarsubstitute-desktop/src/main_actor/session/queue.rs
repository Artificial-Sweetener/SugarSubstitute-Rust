// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy dedicated native session messages into preclaimed FIFO storage before normal-loop decode.
use crate::{DesktopError, main_events::Events};
use std::{cell::RefCell, rc::Rc};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
pub(super) const CAPACITY: usize = 8;
pub(super) const FRAME_BYTES: usize = crate::main_session_contract::MAX_INGRESS_BYTES;
pub(super) const SOURCE_BYTES: usize = 8192;
struct Slot {
    source: String,
    payload: String,
}
struct State {
    slots: Vec<Slot>,
    first: usize,
    count: usize,
    closed: bool,
}
pub(super) struct Queue {
    state: RefCell<State>,
    events: Rc<Events>,
    _memory: MemoryClaim,
}
impl Queue {
    pub(super) fn new(events: Rc<Events>, memory: &MemoryLease) -> Result<Rc<Self>, DesktopError> {
        let claim = memory
            .claim((CAPACITY * (FRAME_BYTES + SOURCE_BYTES + size_of::<Slot>()) + 4096) as u64)
            .map_err(|cause| DesktopError::new("claim_session_queue", cause))?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(CAPACITY)
            .map_err(|cause| DesktopError::new("allocate_session_queue", cause))?;
        for _ in 0..CAPACITY {
            let mut source = String::new();
            let mut payload = String::new();
            source
                .try_reserve_exact(SOURCE_BYTES)
                .map_err(|cause| DesktopError::new("allocate_session_source", cause))?;
            payload
                .try_reserve_exact(FRAME_BYTES)
                .map_err(|cause| DesktopError::new("allocate_session_payload", cause))?;
            if source.capacity() != SOURCE_BYTES || payload.capacity() != FRAME_BYTES {
                return Err(DesktopError::message("allocate_session_queue", "unexpected capacity"));
            }
            slots.push(Slot { source, payload });
        }
        Ok(Rc::new(Self {
            state: RefCell::new(State { slots, first: 0, count: 0, closed: false }),
            events,
            _memory: claim,
        }))
    }
    /// Native getters have returned before either copier reads their owned strings.
    pub(super) fn record(
        &self,
        source: impl FnOnce(&mut String) -> Result<(), DesktopError>,
        payload: impl FnOnce(&mut String) -> Result<(), DesktopError>,
    ) {
        let mut state = self.state.borrow_mut();
        if state.closed {
            return;
        }
        if state.count == CAPACITY {
            tracing::warn!(
                operation = "session_ingress_overload",
                "Dedicated session queue refused overload"
            );
            return;
        }
        let index = (state.first + state.count) % CAPACITY;
        let slot = &mut state.slots[index];
        slot.source.clear();
        slot.payload.clear();
        let result = source(&mut slot.source).and_then(|()| payload(&mut slot.payload));
        if let Err(cause) = result {
            tracing::warn!(operation="session_ingress_refused", cause=%cause,
                "Untrusted workspace input exceeded its native text admission");
        } else {
            state.count += 1;
        }
        drop(state);
        self.events.wake();
    }
    pub(super) fn consume<T>(
        &self,
        consume: impl FnOnce(&str, &str) -> Result<T, DesktopError>,
    ) -> Result<Option<T>, DesktopError> {
        if self.events.in_callback() {
            return Err(DesktopError::message("consume_session_queue", "callback cannot drain"));
        }
        let mut state = self.state.borrow_mut();
        if state.count == 0 {
            return Ok(None);
        }
        let index = state.first;
        let slot = &mut state.slots[index];
        let result = consume(&slot.source, &slot.payload);
        slot.source.clear();
        slot.payload.clear();
        state.first = (state.first + 1) % CAPACITY;
        state.count -= 1;
        result.map(Some)
    }
    pub(super) fn pending(&self) -> bool {
        self.state.borrow().count != 0
    }
    pub(super) fn close(&self) {
        let mut state = self.state.borrow_mut();
        state.closed = true;
        for slot in &mut state.slots {
            slot.payload.clear();
            slot.source.clear();
        }
        state.count = 0;
    }
}

/// Copy only bounded exact UTF-16 returned by a completed native getter, without allocating.
pub(super) fn copy(
    output: &mut String,
    maximum: usize,
    mut read: impl FnMut(usize) -> u16,
) -> Result<(), DesktopError> {
    let length = (0..=maximum).find(|index| read(*index) == 0).ok_or_else(|| {
        DesktopError::message("copy_session_payload", "native text exceeded its bound")
    })?;
    let mut bytes = 0;
    for value in char::decode_utf16((0..length).map(&mut read)) {
        bytes +=
            value.map_err(|cause| DesktopError::new("copy_session_payload", cause))?.len_utf8();
        if bytes > maximum {
            return Err(DesktopError::message(
                "copy_session_payload",
                "UTF-8 text exceeded its bound",
            ));
        }
    }
    if output.capacity() < maximum {
        return Err(DesktopError::message("copy_session_payload", "text storage was not admitted"));
    }
    for value in char::decode_utf16((0..length).map(read)) {
        output.push(value.map_err(|cause| DesktopError::new("copy_session_payload", cause))?);
    }
    Ok(())
}
#[cfg(test)]
mod tests;
