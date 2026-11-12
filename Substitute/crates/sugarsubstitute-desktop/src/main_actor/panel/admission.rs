// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Issue three fixed transport permits backed by their original allocation custody.

use crate::main_panel_ingress::{Lane, Ticket, TransportAction, TransportActionKind};
use crate::{DesktopError, main_native_delivery::NativeDelivery};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

const LANES: [Lane; 3] = [Lane::Control, Lane::Ordinary, Lane::Cancellation];
const STORAGE_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Ready,
    Issued,
    Refuse,
    Refusing,
    Reject,
    Rejecting,
    Application,
    Delivering,
}

struct Slot {
    phase: Phase,
    ticket: Ticket,
    request_id: Option<u64>,
    claim: Option<MemoryClaim>,
    grant_pending: bool,
}

/// One issuer owns admission; application services retain typed commands and decisions.
pub(in crate::main_actor) struct Admission {
    slots: [Slot; 3],
    next: u64,
    context: Option<i64>,
    closed: bool,
    cursor: usize,
    _decode_claim: MemoryClaim,
}

impl Admission {
    /// Claim every lane before advertising any capacity to the browser.
    pub(in crate::main_actor) fn new(lease: &MemoryLease) -> Result<Self, DesktopError> {
        let slot = || -> Result<Slot, DesktopError> {
            Ok(Slot {
                phase: Phase::Ready,
                ticket: Ticket(0),
                request_id: None,
                claim: Some(
                    lease
                        .claim(STORAGE_BYTES)
                        .map_err(|cause| DesktopError::new("claim_panel_lane", cause))?,
                ),
                grant_pending: false,
            })
        };
        Ok(Self {
            slots: [slot()?, slot()?, slot()?],
            next: 1,
            context: None,
            closed: false,
            cursor: 0,
            _decode_claim: lease
                .claim(STORAGE_BYTES)
                .map_err(|cause| DesktopError::new("claim_panel_decode", cause))?,
        })
    }

    /// Fence a changed native document instead of letting its old tickets reach another page.
    pub(in crate::main_actor) fn synchronize(&mut self, context: Option<i64>) {
        if self.context.is_some() && self.context != context {
            self.closed = true;
        }
        if self.context.is_none() && !self.closed {
            self.context = context;
        }
    }

    /// Close admission irreversibly while outstanding application/native owners retain their credit.
    pub(in crate::main_actor) fn close(&mut self) {
        self.closed = true;
    }

    /// Identify finite ready/refusal work without polling pending application or native effects.
    pub(in crate::main_actor) fn needs_progress(&self) -> bool {
        !self.closed
            && self.context.is_some()
            && self.slots.iter().any(|slot| {
                !slot.grant_pending
                    && matches!(slot.phase, Phase::Ready | Phase::Refuse | Phase::Reject)
            })
    }

    /// Arm a grant/refusal before a possibly synchronous native call can invoke JavaScript.
    pub(super) fn action(&mut self) -> Result<Option<TransportAction>, DesktopError> {
        if !self.needs_progress() {
            return Ok(None);
        }
        for offset in 0..LANES.len() {
            let index = (self.cursor + offset) % LANES.len();
            let lane = LANES[index];
            let slot = &mut self.slots[lane.index()];
            if slot.grant_pending {
                continue;
            }
            if matches!(slot.phase, Phase::Refuse | Phase::Reject) {
                let kind = if slot.phase == Phase::Refuse {
                    slot.phase = Phase::Refusing;
                    TransportActionKind::RetryableRefusal
                } else {
                    slot.phase = Phase::Rejecting;
                    TransportActionKind::PermanentRejection
                };
                self.cursor = (index + 1) % LANES.len();
                return Ok(Some(TransportAction { lane, ticket: slot.ticket, kind }));
            }
            if slot.phase == Phase::Ready {
                if slot.claim.is_none() {
                    return Err(fault("ready lane lost its credit"));
                }
                if self.next > 9007199254740991 {
                    return Err(fault("ticket identity exhausted"));
                }
                slot.ticket = Ticket(self.next);
                self.next += 1;
                slot.phase = Phase::Issued;
                slot.grant_pending = true;
                self.cursor = (index + 1) % LANES.len();
                return Ok(Some(TransportAction {
                    lane,
                    ticket: slot.ticket,
                    kind: TransportActionKind::Grant,
                }));
            }
        }
        Ok(None)
    }

    /// Observe an actual successful private command without inferring application completion.
    pub(in crate::main_actor) fn action_completed(
        &mut self,
        context: i64,
        action: TransportAction,
    ) -> Result<(), DesktopError> {
        if self.context != Some(context) || self.closed {
            return Ok(());
        }
        let slot = &mut self.slots[action.lane.index()];
        if slot.ticket != action.ticket {
            return Err(fault("foreign transport completion"));
        }
        match action.kind {
            TransportActionKind::Grant => {
                if !slot.grant_pending {
                    return Err(fault("duplicate grant completion"));
                }
                slot.grant_pending = false;
            }
            TransportActionKind::RetryableRefusal | TransportActionKind::PermanentRejection => {
                let expected = if action.kind == TransportActionKind::RetryableRefusal {
                    Phase::Refusing
                } else {
                    Phase::Rejecting
                };
                if slot.phase != expected {
                    return Err(fault("duplicate refusal completion"));
                }
                slot.phase = Phase::Ready;
            }
        }
        Ok(())
    }

    /// Validate a ticket before decoding any public operation under its retained claim.
    pub(in crate::main_actor) fn issued(&self, context: i64, lane: Lane, ticket: Ticket) -> bool {
        !self.closed
            && self.context == Some(context)
            && self.slots[lane.index()].phase == Phase::Issued
            && self.slots[lane.index()].ticket == ticket
    }

    /// Transfer the same preclaimed credit only after context, session and public shape validation.
    pub(in crate::main_actor) fn consume(
        &mut self,
        context: i64,
        lane: Lane,
        ticket: Ticket,
        request_id: Option<u64>,
    ) -> Result<Option<MemoryClaim>, DesktopError> {
        if !self.issued(context, lane, ticket) {
            return Ok(None);
        }
        if lane == Lane::Cancellation && self.slots[Lane::Ordinary.index()].request_id != request_id
        {
            return Ok(None);
        }
        let slot = &mut self.slots[lane.index()];
        let claim = slot.claim.take().ok_or_else(|| fault("issued ticket lost its credit"))?;
        slot.phase = Phase::Application;
        slot.request_id = request_id;
        Ok(Some(claim))
    }

    /// Consume invalid input without retrying the same malformed operation.
    pub(in crate::main_actor) fn reject(&mut self, lane: Lane, ticket: Ticket) {
        let slot = &mut self.slots[lane.index()];
        if slot.phase == Phase::Issued && slot.ticket == ticket {
            slot.phase = Phase::Reject;
        }
    }

    /// Coalesce raw callback loss into bounded refusals for issued, unconsumed tickets.
    pub(in crate::main_actor) fn overflow(&mut self) {
        for slot in &mut self.slots {
            if slot.phase == Phase::Issued {
                slot.phase = Phase::Refuse;
            }
        }
    }

    /// Bind one typed response/receipt to the lane that transferred its original allocation.
    pub(super) fn begin_delivery(
        &mut self,
        delivery: &NativeDelivery,
    ) -> Result<Ticket, DesktopError> {
        let lane = delivery.lane().ok_or_else(|| fault("notification owns no transport lane"))?;
        let slot = &mut self.slots[lane.index()];
        if slot.phase != Phase::Application || slot.request_id != delivery.request_id() {
            return Err(fault("response does not own an admitted request"));
        }
        slot.phase = Phase::Delivering;
        Ok(slot.ticket)
    }

    /// Return credit after actual callback return and actor validation, never on submission.
    pub(in crate::main_actor) fn completed(
        &mut self,
        ticket: Ticket,
        delivery: NativeDelivery,
    ) -> Result<(), DesktopError> {
        let lane = delivery.lane().ok_or_else(|| fault("notification owns no transport lane"))?;
        let slot = &mut self.slots[lane.index()];
        if slot.phase != Phase::Delivering
            || slot.ticket != ticket
            || slot.request_id != delivery.request_id()
        {
            return Err(fault("delivery completion does not own its lane"));
        }
        slot.claim = Some(delivery.into_claim());
        slot.request_id = None;
        slot.phase = Phase::Ready;
        Ok(())
    }
}

/// Keep admission failures actionable while preserving existing native shutdown behavior.
fn fault(message: &'static str) -> DesktopError {
    DesktopError::message("panel_transport_admission", message)
}

#[cfg(test)]
mod tests;
