// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded UI-affine media deferrals while the application owns cancellable I/O jobs.
mod admission;
mod finish;
#[cfg(test)]
mod tests;
use super::*;
use crate::{
    DesktopStartupObservation, MainIdentity, SessionBootstrap, SessionMediaObservation,
    SessionMediaReply,
    main_session_media::{
        MediaResponse, SessionMediaFailure,
        route::{Endpoint, Evidence, Requested},
    },
};
use std::{
    cell::RefCell,
    num::NonZeroU64,
    time::{Duration, Instant},
};
const CAPACITY: usize = 10;
const ACTIVE: usize = 2;
const GET_CAPACITY: usize = CAPACITY - 1;
const LIFETIME: Duration = Duration::from_secs(15);
const REQUEST_BYTES: u64 = 32 * 1024;
#[allow(
    clippy::large_enum_variant,
    reason = "Ten media deferral slots are preallocated under one 64 KiB claim; inline state keeps native callback transitions allocation-free while preserving completion custody"
)]
enum Slot {
    Vacant,
    Opening { id: NonZeroU64, head: bool },
    Pending(Pending),
    Completing { id: NonZeroU64, head: bool },
}
struct Pending {
    id: NonZeroU64,
    request: Option<SessionMediaObservation>,
    args: ICoreWebView2WebResourceRequestedEventArgs,
    deferral: ICoreWebView2Deferral,
    environment: ICoreWebView2Environment,
    deadline: Instant,
    range: Option<crate::SessionMediaRange>,
    head: bool,
    active: bool,
    cancelled: bool,
    cancel_sent: bool,
    response: Option<Result<MediaResponse, SessionMediaFailure>>,
    responded: bool,
}
struct State {
    slots: Vec<Slot>,
    next: u64,
    closed: bool,
    failure: Option<DesktopError>,
}
impl State {
    fn get_occupancy(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| match slot {
                Slot::Pending(value) => !value.head,
                Slot::Opening { head, .. } | Slot::Completing { head, .. } => !head,
                Slot::Vacant => false,
            })
            .count()
    }
    /// A held GET must leave native execution and admission credit for later metadata.
    fn next_request(&self) -> Option<usize> {
        if self.closed
            || self
                .slots
                .iter()
                .filter(|slot| matches!(slot,Slot::Pending(value) if value.active))
                .count()
                >= ACTIVE
        {
            return None;
        }
        let head = self.slots.iter().position(|slot| {
            matches!(slot,Slot::Pending(value)
            if !value.active && value.head && value.request.is_some())
        });
        if head.is_some() {
            return head;
        }
        if self
            .slots
            .iter()
            .any(|slot| matches!(slot,Slot::Pending(value) if value.active && !value.head))
        {
            return None;
        }
        self.slots.iter().position(|slot| {
            matches!(slot,Slot::Pending(value)
            if !value.active && !value.head && value.request.is_some())
        })
    }
}
/// Sole retained-media request registry shared with the existing reserved-resource handler.
pub(crate) struct Media {
    identity: MainIdentity,
    bootstrap: SessionBootstrap,
    endpoint: Endpoint,
    state: RefCell<State>,
    events: Rc<Events>,
    memory: MemoryLease,
    _memory: MemoryClaim,
}
pub(crate) struct Progress {
    pub(crate) consumed: bool,
    pub(crate) observation: Option<DesktopStartupObservation>,
}
impl Media {
    pub(crate) fn new(
        identity: MainIdentity,
        bootstrap: SessionBootstrap,
        events: Rc<Events>,
        memory: &MemoryLease,
    ) -> Result<Self, DesktopError> {
        let claim = memory
            .claim(64 * 1024)
            .map_err(|cause| DesktopError::new("claim_session_media", cause))?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(CAPACITY)
            .map_err(|cause| DesktopError::new("allocate_session_media", cause))?;
        slots.resize_with(CAPACITY, || Slot::Vacant);
        Ok(Self {
            identity,
            endpoint: Endpoint::new(bootstrap.clone(), super::random_token()?),
            bootstrap,
            state: RefCell::new(State { slots, next: 0, closed: false, failure: None }),
            events,
            memory: memory.clone(),
            _memory: claim,
        })
    }
    pub(crate) fn path(&self) -> &str {
        self.endpoint.path()
    }
    pub(crate) fn filter(&self) -> &str {
        self.endpoint.filter()
    }
    pub(crate) fn matches_family(&self, uri: &str) -> bool {
        self.endpoint.matches_family(uri)
    }
    /// Accept a current worker result; a late response is an explicit inert miss.
    pub(crate) fn reply(&self, reply: SessionMediaReply) -> Result<(), DesktopError> {
        if reply.identity() != self.identity {
            return Ok(());
        }
        let mut state = self.state.borrow_mut();
        let Some(pending) = state.slots.iter_mut().find_map(|slot| match slot {
            Slot::Pending(value) if value.id == reply.request_id() => Some(value),
            _ => None,
        }) else {
            return Ok(());
        };
        if pending.cancelled || pending.response.is_some() || pending.responded {
            return Ok(());
        }
        pending.response = Some(reply.into_body());
        drop(state);
        self.events.wake();
        Ok(())
    }
    pub(crate) fn needs_progress(&self, now: Instant) -> bool {
        let state = self.state.borrow();
        state.failure.is_some() || state.next_request().is_some()
            || state.slots.iter().any(|slot|matches!(slot,Slot::Pending(value)
                if value.response.is_some() || value.responded || value.cancelled || state.closed || now>=value.deadline))
    }
    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.state
            .borrow()
            .slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Pending(value) => Some(value.deadline),
                _ => None,
            })
            .min()
    }
    pub(crate) fn close(&self) {
        let mut state = self.state.borrow_mut();
        state.closed = true;
        for slot in &mut state.slots {
            if let Slot::Pending(value) = slot {
                value.cancelled = true;
            }
        }
        drop(state);
        self.events.wake();
    }
    pub(crate) fn retired(&self) -> bool {
        let state = self.state.borrow();
        state.closed && state.slots.iter().all(|slot| matches!(slot, Slot::Vacant))
    }
    /// Advance at most one native response, cancellation observation or queued job admission.
    pub(crate) fn step(&self, now: Instant) -> Result<Progress, DesktopError> {
        if self.events.in_callback() {
            return Err(DesktopError::message(
                "progress_session_media",
                "callback cannot drive I/O",
            ));
        }
        let mut state = self.state.borrow_mut();
        if let Some(cause) = state.failure.take() {
            return Err(cause);
        }
        let closed = state.closed;
        for slot in &mut state.slots {
            if let Slot::Pending(value) = slot {
                if closed || now >= value.deadline {
                    value.cancelled = true;
                }
                if value.cancelled && value.active && !value.cancel_sent {
                    value.cancel_sent = true;
                    return Ok(Progress {
                        consumed: true,
                        observation: Some(DesktopStartupObservation::SessionMediaCancelled {
                            identity: self.identity,
                            request_id: value.id,
                        }),
                    });
                }
            }
        }
        if let Some(index) = state.slots.iter().position(
            |slot| matches!(slot,Slot::Pending(value) if value.cancelled||value.response.is_some()||value.responded),
        ) {
            let (id,head) = match &state.slots[index] {
                Slot::Pending(value) => (value.id,value.head),
                _ => {
                    return Err(DesktopError::message(
                        "session_media_finish",
                        "pending slot disappeared",
                    ));
                }
            };
            let Slot::Pending(mut pending) =
                std::mem::replace(&mut state.slots[index], Slot::Completing {id,head})
            else {
                return Err(DesktopError::message(
                    "session_media_finish",
                    "native slot disappeared",
                ));
            };
            drop(state);
            let result = pending.finish();
            let mut state = self.state.borrow_mut();
            if !matches!(state.slots[index],Slot::Completing {id:value,..} if value==id) {
                return Err(DesktopError::message(
                    "session_media_finish",
                    "completion slot changed",
                ));
            }
            state.slots[index] = if result.is_ok() { Slot::Vacant } else { Slot::Pending(pending) };
            result?;
            return Ok(Progress { consumed: true, observation: None });
        }
        if let Some(index) = state.next_request() {
            let Slot::Pending(value) = &mut state.slots[index] else {
                return Err(DesktopError::message(
                    "session_media_admission",
                    "selected request disappeared",
                ));
            };
            let request = value.request.take().ok_or_else(|| {
                DesktopError::message(
                    "session_media_admission",
                    "selected request is already active",
                )
            })?;
            value.active = true;
            return Ok(Progress {
                consumed: true,
                observation: Some(DesktopStartupObservation::SessionMediaRequested(request)),
            });
        }
        Ok(Progress { consumed: false, observation: None })
    }
}
