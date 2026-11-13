// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Observe one original preparation's native custody without owning native authority.

use crate::scope::lock;
use std::sync::{Arc, Mutex};
#[cfg(any(windows, test))]
use sugarsubstitute_contracts::execution::ExecutionId;
use sugarsubstitute_contracts::execution::{JobId, ScopeId};

/// Closed observations published by the actual outer environment owner.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum PreparationPhase {
    #[cfg(any(windows, test))]
    Prepared,
    #[cfg(any(windows, test))]
    GetterInFlight,
    #[cfg(any(windows, test))]
    Held,
    #[cfg(any(windows, test))]
    CopyInFlight,
    #[cfg(any(windows, test))]
    FreeInFlight,
    #[cfg(any(windows, test))]
    Released,
    #[cfg(any(windows, test))]
    Delivered,
    Uncertain,
}

/// Passive metadata only: no pointer, port, context, claim or provider callback.
pub(crate) struct PreparationSlot {
    #[cfg(any(windows, test))]
    pub(crate) execution: ExecutionId,
    pub(crate) scope: ScopeId,
    pub(crate) job: JobId,
    pub(crate) state: Mutex<PreparationState>,
}

/// Keep terminal acknowledgement independent of actual native settlement.
pub(crate) struct PreparationState {
    pub(crate) phase: PreparationPhase,
    pub(crate) callback_quiescent: bool,
    pub(crate) native_handoff: bool,
    pub(crate) native_retirement_active: bool,
    pub(crate) pending_result_disposal: bool,
}

impl PreparationSlot {
    /// Construct only after the genuine original full claim is adopted.
    #[cfg(any(windows, test))]
    pub(crate) fn new(execution: ExecutionId, scope: ScopeId, job: JobId) -> Self {
        Self {
            execution,
            scope,
            job,
            state: Mutex::new(PreparationState {
                phase: PreparationPhase::Prepared,
                callback_quiescent: true,
                native_handoff: false,
                native_retirement_active: false,
                pending_result_disposal: false,
            }),
        }
    }

    /// Sample only counter/phase metadata; this cannot call or release native work.
    pub(crate) fn held(&self) -> bool {
        let state = lock(&self.state);
        !state.callback_quiescent
            || state.native_handoff
            || state.native_retirement_active
            || state.phase.held()
    }
}

impl PreparationPhase {
    /// Native work or an original allocation remains outstanding in this phase.
    fn held(self) -> bool {
        match self {
            #[cfg(any(windows, test))]
            Self::GetterInFlight | Self::Held | Self::CopyInFlight | Self::FreeInFlight => true,
            Self::Uncertain => true,
            #[cfg(any(windows, test))]
            Self::Prepared | Self::Released | Self::Delivered => false,
        }
    }
}

/// Coalesce abandoned result disposal into this job's already-held native entry.
/// Only passive metadata changes under Core; actual destruction occurs on its worker.
pub(crate) fn defer_disposal(slot: Option<&Arc<PreparationSlot>>) -> bool {
    let Some(slot) = slot else {
        return false;
    };
    let mut state = lock(&slot.state);
    if state.native_handoff || state.native_retirement_active {
        state.pending_result_disposal = true;
        true
    } else {
        false
    }
}
