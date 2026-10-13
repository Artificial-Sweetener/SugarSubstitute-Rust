// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual producer replies through failed recipient delivery without replaying effects.
use super::{Result, retirement::RetiredDesktop};
use sugarsubstitute_contracts::lifecycle::{ResourceTransition, RunKey};

/// A whole returned transition remains charged by its source record until every
/// correlated recipient accepts it or its actual desktop retirement is observed.
pub(super) struct Forwarded {
    transition: ResourceTransition,
    delivered: [Option<RunKey>; 64],
}
impl Forwarded {
    pub(super) fn new(transition: ResourceTransition) -> Self {
        Self { transition, delivered: [None; 64] }
    }
    /// Invoke the real recipient only once after acceptance. A rejected recipient
    /// leaves both the complete original transition and its remaining work owned.
    pub(super) fn deliver(
        &mut self,
        run: RunKey,
        receive: impl FnOnce(&ResourceTransition) -> Result<()>,
    ) -> Result<()> {
        if self.delivered.contains(&Some(run))
            || !self.transition.replies.iter().any(|reply| reply.run == run)
        {
            return Ok(());
        }
        let at = self
            .delivered
            .iter()
            .position(Option::is_none)
            .ok_or("Forwarded recipient bound occupied")?;
        receive(&self.transition)?;
        self.delivered[at] = Some(run);
        Ok(())
    }
    /// Actual same-run native retirement discharges only the unreachable reply.
    pub(super) fn retire(&mut self, retired: &RetiredDesktop) -> Result<()> {
        self.deliver(retired.run(), |_| Ok(()))
    }
    pub(super) fn complete(&self) -> bool {
        self.transition.replies.iter().all(|reply| self.delivered.contains(&Some(reply.run)))
    }
}

#[cfg(test)]
#[path = "forwarding_tests.rs"]
mod tests;
