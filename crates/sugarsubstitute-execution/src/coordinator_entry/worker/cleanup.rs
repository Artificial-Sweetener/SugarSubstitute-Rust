// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Freeze the first private cleanup decision while preserving root Serving shutdown authority.
use super::*;
pub(in crate::coordinator_entry) enum Cleanup {
    Pending,
    RootServing,
    Finite { budget: ShutdownBudget, expired: bool },
    Failed,
}
#[cfg(test)]
#[path = "../cleanup_tests.rs"]
mod tests;
impl Cleanup {
    pub(super) fn begin(
        &mut self,
        serving: bool,
        now: Moment,
        clock_available: bool,
    ) -> Result<(), EntryTransportError> {
        if !matches!(self, Self::Pending) {
            return Ok(());
        }
        if serving {
            *self = Self::RootServing;
        } else {
            // A failed first decision is latched before arithmetic; later observations cannot retry it.
            *self = Self::Failed;
            if clock_available {
                let budget = ShutdownBudget::new(now).map_err(|cause| {
                    EntryTransportError::native(NativeObservationError::new(
                        "entry_cleanup_budget",
                        cause,
                    ))
                })?;
                *self = Self::Finite { budget, expired: false };
            }
        }
        Ok(())
    }
    pub(super) fn deadline(&self, now: Moment, clock_available: bool) -> Option<Moment> {
        match self {
            Self::Finite { budget, expired: false } if clock_available => {
                if now < budget.graceful_until {
                    Some(budget.graceful_until)
                } else if now < budget.settle_until {
                    Some(budget.settle_until)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    pub(super) fn take_expiry(&mut self, now: Moment, clock_available: bool) -> bool {
        if let Self::Finite { budget, expired } = self
            && clock_available
            && !*expired
            && now >= budget.settle_until
        {
            *expired = true;
            return true;
        }
        false
    }
}
