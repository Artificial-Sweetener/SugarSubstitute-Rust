// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Revoke only the exact fixed write admission, without native cancellation or clocks.
use super::*;
#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Queued,
    Revoked,
    Issuing(Moment),
    Issued { at: Moment, eligible: bool },
    Retired,
}
pub(crate) struct Admission {
    pub(crate) operation: LifecyclePipeOperationId,
    pub(crate) phase: Phase,
}
/// Opaque passive identity for one admitted write. Drop does not revoke it.
/// Revocation is thread-safe and never exposes native handles or scheduler access.
pub struct NativeWriteAdmission {
    pub(crate) shared: SharedRef,
    pub(crate) operation: LifecyclePipeOperationId,
}
/// Distinguish a proven no-issue veto from historical native acceptance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWriteRevocation {
    /// This call vetoed the queued write before any native issue.
    RevokedBeforeIssue,
    /// The exact queued write was already vetoed.
    AlreadyRevoked,
    /// Native acceptance preceded revocation; native I/O and deadline remain live.
    AlreadyIssued {
        /// Original local execution-clock sample immediately before native issue.
        at: Moment,
    },
    /// The native owner unwound during issue; actual retirement must settle its custody.
    IssueUncertain {
        /// Original sample before the attempted native call, not acceptance proof.
        at: Moment,
    },
    /// The identity is stale or retired without accepted native issue.
    Retired,
}
impl NativeWriteAdmission {
    /// Return this connection-local identity as metadata, without native authority.
    pub const fn operation(&self) -> LifecyclePipeOperationId {
        self.operation
    }
    /// Veto this exact pending issue, or invalidate its application eligibility.
    /// AlreadyIssued never cancels a partial message, closes the channel or rearms time.
    pub fn revoke(&self) -> NativeWriteRevocation {
        if let Some(core) = self.shared.core.upgrade() {
            let mut state = lock(&core.state);
            let result = self.revoke_with_core(&mut state);
            drop(state);
            core.changed();
            result
        } else {
            self.revoke_locked(&mut lock(&self.shared.state))
        }
    }
    pub(super) fn revoke_with_core(
        &self,
        state: &mut crate::admission::State,
    ) -> NativeWriteRevocation {
        let mut pipe = lock(&self.shared.state);
        let result = self.revoke_locked(&mut pipe);
        if result == NativeWriteRevocation::RevokedBeforeIssue
            && let Some(slots) = state
                .jobs
                .get_mut(&self.shared.id)
                .and_then(|record| record.native_deadlines.as_mut())
        {
            slots.clear(Role::Write, self.operation.get());
        }
        result
    }
    fn revoke_locked(&self, pipe: &mut State) -> NativeWriteRevocation {
        let Some(admission) = &mut pipe.write_admission else {
            return NativeWriteRevocation::Retired;
        };
        if admission.operation != self.operation {
            return NativeWriteRevocation::Retired;
        }
        match admission.phase {
            Phase::Issued { at, eligible } => {
                if eligible {
                    admission.phase = Phase::Issued { at, eligible: false };
                }
                NativeWriteRevocation::AlreadyIssued { at }
            }
            Phase::Issuing(at) => NativeWriteRevocation::IssueUncertain { at },
            Phase::Revoked => NativeWriteRevocation::AlreadyRevoked,
            Phase::Queued if !pipe.stopped => {
                admission.phase = Phase::Revoked;
                NativeWriteRevocation::RevokedBeforeIssue
            }
            Phase::Queued | Phase::Retired => NativeWriteRevocation::Retired,
        }
    }
}
