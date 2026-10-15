// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the actual funded coordinator from source installation through worker return and adoption.
use super::super::service::CollectionIssue;
use crate::production::incident_host::FundedState;

/// One original policy is retained locally or exclusively issued to the actual source loan.
pub(super) struct PolicyCustody {
    retained: Option<FundedState>,
    issued: bool,
}
impl PolicyCustody {
    pub(super) const fn new() -> Self {
        Self { retained: None, issued: false }
    }
    /// Keep the original funded value; an occupied source never replaces its custody.
    pub(super) fn install(
        &mut self,
        input: &mut Option<FundedState>,
    ) -> Result<(), CollectionIssue> {
        if self.pending() || input.is_none() {
            return Err(CollectionIssue::Busy);
        }
        self.retained = input.take();
        Ok(())
    }
    /// Lend the identical value once; only its constructor-private loan can return it.
    pub(super) fn issue(&mut self) -> Result<FundedState, CollectionIssue> {
        if self.issued {
            return Err(CollectionIssue::Busy);
        }
        let policy = self.retained.take().ok_or(CollectionIssue::Busy)?;
        self.issued = true;
        Ok(policy)
    }
    /// Close the unique issued move; the real service still requires matched terminal adoption.
    pub(super) fn return_issued(&mut self, policy: FundedState) {
        self.retained = Some(policy);
        self.issued = false;
    }
    /// Borrow the returned value without lending its coordinator or claiming native retirement.
    pub(super) fn retained(&self) -> Option<&FundedState> {
        self.retained.as_ref()
    }
    /// Source custody remains outstanding after worker return until the same value is restored.
    pub(super) fn pending(&self) -> bool {
        self.issued || self.retained.is_some()
    }
    /// Restore only a returned value into an empty caller slot after its existing owner fence.
    pub(super) fn restore(
        &mut self,
        output: &mut Option<FundedState>,
    ) -> Result<(), CollectionIssue> {
        if self.issued || output.is_some() || self.retained.is_none() {
            return Err(CollectionIssue::Busy);
        }
        *output = self.retained.take();
        Ok(())
    }
}
