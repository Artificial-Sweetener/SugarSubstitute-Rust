// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish original queued write revocation from issued native I/O completion.
use super::ParentError;

/// Facts delivered only after the containing actor validates its actual native receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum WriteObservation {
    Queued,
    Issued,
    Written,
    Revoked,
}

#[derive(Clone, Copy)]
enum State {
    Unqueued,
    Queued,
    Issued,
    Written,
    Revoked,
    Withdrawn,
}

/// One final acquisition write; no metadata transition manufactures a native receipt.
pub(super) struct OriginalWrite(State);
impl OriginalWrite {
    pub(super) const fn new() -> Self {
        Self(State::Unqueued)
    }
    /// Observe exact native queue/issue/completion or never-issued revocation ordering.
    pub(super) fn observe(&mut self, observation: WriteObservation) -> Result<(), ParentError> {
        self.0 = match (self.0, observation) {
            (State::Unqueued, WriteObservation::Queued) => State::Queued,
            (State::Queued, WriteObservation::Issued) => State::Issued,
            (State::Issued, WriteObservation::Written) => State::Written,
            (State::Queued, WriteObservation::Revoked) => State::Revoked,
            _ => return Err(ParentError::WrongPhase),
        };
        Ok(())
    }
    /// Withdraw only a reply never accepted by the native write owner.
    pub(super) fn withdraw_unqueued(&mut self) {
        if matches!(self.0, State::Unqueued) {
            self.0 = State::Withdrawn;
        }
    }
    pub(super) const fn written(&self) -> bool {
        matches!(self.0, State::Written)
    }
    pub(super) const fn pending(&self, retiring: bool) -> bool {
        matches!(self.0, State::Queued | State::Issued)
            || (!retiring && matches!(self.0, State::Unqueued))
    }
    /// No active native write remains; an unqueued reply owns no native operation.
    pub(super) const fn closed(&self) -> bool {
        !matches!(self.0, State::Queued | State::Issued)
    }
}
